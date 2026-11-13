# Orquestra CLI — Blind Credential Vault (`vault`)

The `orquestra vault` subcommand family provides granular command-line management, zero-leak credential pooling, health monitoring, security audits, and blind proxy execution (HTTP and child processes) for the OrquestraAI Blind Vault subsystem.

---

## 1. Overview & Architecture

Modern AI agentic workflows require access to cloud providers, third-party APIs, and deployment infrastructure (e.g., Cloudflare, OpenAI, Anthropic, GitHub, AWS, Stripe). However, passing plaintext API keys directly into an AI agent's prompt or context window poses severe credential leakage risks (accidental transcript persistence, unauthorized exfiltration, prompt injection).

OrquestraAI resolves this with the **Blind Vault** pattern:

- **Zero-Context Secrets:** Secrets are sealed at rest and injected into outgoing HTTP calls or subprocess environments entirely within the native Rust backend. The agent interacts with credentials purely through abstract service IDs and blind proxy endpoints.
- **Scrubbed Output:** All response bodies, error messages, `stdout`, and `stderr` streams are scrubbed against all registered keys before returning to the caller.
- **Failover Pools & Cooldowns:** Services support pools of multiple API keys with automatic failover on 401/402/403/429 errors, tagging (`workers`, `d1`, etc.), and health tracking (`healthy`, `degraded`, `cooldown`).
- **At-Rest AES-256-GCM Encryption:** Credentials in `~/.orquestra/vault.json` are sealed under a per-installation key held in the OS keyring (Secret Service, macOS Keychain, Windows Credential Manager) or a `0600` headless fallback file.

```
┌────────────────────────────────────────────────────────────────────────┐
│                        AI Agent / CLI Operator                         │
└──────────────────────────────────┬─────────────────────────────────────┘
                                   │ orquestra vault <cmd>
                                   ▼
┌────────────────────────────────────────────────────────────────────────┐
│                Orquestra Blind Vault (Native Rust Core)                │
│                                                                        │
│  ┌──────────────────────┐  ┌───────────────────┐  ┌─────────────────┐  │
│  │ AES-256-GCM Envelope │  │ SSRF / URL Gate   │  │ Exec Allowlist  │  │
│  │ (Keyring / Fallback) │  │ (Presets Catalog) │  │ (Bare Binaries) │  │
│  └──────────────────────┘  └───────────────────┘  └─────────────────┘  │
│                                                                        │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │ Memory Secret Injection & Key Pool Failover Engine               │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                   │                                    │
│             Scrubbed Text Only    │ Scrubbed Output Streams            │
└───────────────────────────────────┼────────────────────────────────────┘
                                    │
            ┌───────────────────────┴───────────────────────┐
            ▼                                               ▼
┌───────────────────────┐                       ┌───────────────────────┐
│ Target Remote API     │                       │ Isolated Subprocess   │
│ (https://api.github…) │                       │ (gh, aws, doctl...)   │
└───────────────────────┘                       └───────────────────────┘
```

---

## 2. Current State vs. Granular CLI Design

| Capability               | Current State (`src-tauri/src/cli/native/vault.rs`) | New Granular Design (`orquestra vault <cmd>`)                                                   |
| :----------------------- | :-------------------------------------------------- | :---------------------------------------------------------------------------------------------- |
| **List Services**        | Basic `vault list [--json]`                         | `orquestra vault list [--preset <P>] [--enabled-only] [--failing-only] [--search <S>] [--json]` |
| **Inspect Service**      | _None_ (requires reading sealed file)               | `orquestra vault inspect <id> [--reveal] [--json]`                                              |
| **Add / Update Service** | _None_ (UI modal only)                              | `orquestra vault set <id> --key <K> [--preset <P>] [--meta <K=V>] [--tag <T>] [--json]`         |
| **Add Key to Pool**      | _None_ (UI modal only)                              | `orquestra vault add-key <id> --key <K> [--tag <T>] [--json]`                                   |
| **Remove Key from Pool** | _None_ (UI only)                                    | `orquestra vault remove-key <id> --key-number <N> [--json]`                                     |
| **Delete Service**       | _None_ (UI only)                                    | `orquestra vault remove <id> [--yes] [--json]`                                                  |
| **Kill-Switch Toggle**   | `vault set-enabled <id> <on\|off> [--json]`         | `orquestra vault enable <id>`, `disable <id>`, `toggle <id>`, and `set-enabled`                 |
| **Set Primary Key**      | _None_ (UI only)                                    | `orquestra vault set-primary <id> --key-number <N> [--json]`                                    |
| **Rotate Key**           | _None_                                              | `orquestra vault rotate <id> [--next] [--key-number <N>] [--new-key <K>] [--json]`              |
| **Test Connectivity**    | _None_ (UI only)                                    | `orquestra vault test <id> [--key-number <N>] [--json]`                                         |
| **Blind HTTP Proxy**     | `vault http --service <S> --endpoint <E> ...`       | `orquestra vault proxy-http` (with `http` alias)                                                |
| **Blind Exec Proxy**     | `vault exec <S> [--] <argv...>`                     | `orquestra vault proxy-exec` (with `exec` alias)                                                |
| **Preset Catalog**       | _None_ (UI only)                                    | `orquestra vault presets [--json]`                                                              |
| **Security Audit**       | _None_                                              | `orquestra vault audit [--json]` & `audit purge-backups [--confirm]`                            |

---

## 3. Subcommand Reference

### 3.1 `orquestra vault list`

Lists all credential services registered in the vault, displaying their active health, key counts, enabled state, and masked keys.

```bash
orquestra vault list [OPTIONS]
```

**Options:**

- `--preset <name>`: Filter by provider preset (e.g., `openai`, `github`, `cloudflare`, `custom`).
- `--enabled-only`: Show only enabled services (kill-switch active).
- `--failing-only`: Show only degraded or cooldown services needing attention.
- `--search <term>`: Case-insensitive filter matching service ID, label, or description.
- `--json`: Output full array of `VaultServicePublic` objects in JSON.

**Human Output Example:**

```text
SERVICE ID    LABEL              PRESET      STATUS    HEALTH    KEYS  ACTIVE
openai        OpenAI Production  openai      enabled   Healthy   3     #1
anthropic     Anthropic Primary  anthropic   enabled   Healthy   2     #1
cloudflare    Cloudflare Fleet   cloudflare  enabled   Degraded  4     #2
github        GitHub Automation  github      disabled  Healthy   1     #1
```

---

### 3.2 `orquestra vault inspect <id>`

Inspects detailed metadata and key status for a single vault service. Alias: `show`, `info`.

```bash
orquestra vault inspect <id> [OPTIONS]
```

**Options:**

- `--reveal`: Reveal the cleartext credential secret for the primary key (or keys). Requires an authenticated terminal session.
- `--key-number <N>`: Target a specific 1-based key number when using `--reveal`.
- `--json`: Output as structured JSON.

**Human Output Example:**

```text
Service:     cloudflare
Label:       Cloudflare Fleet
Preset:      cloudflare
State:       Enabled (Kill-switch: OFF)
Health:      Degraded
Description: Production edge workers and D1 database credentials
Metadata:
  account_id: 84a91c78e3294801fa87b321a

Key Pool (4 keys):
  #1 [workers]  key-0: ••••••••••••••••••••8Fa1  (Healthy)
  #2 [d1]       key-1: ••••••••••••••••••••92Xp  (Cooldown, retry in 42s) [PRIMARY]
  #3 [dns]      key-2: ••••••••••••••••••••4Kc7  (Healthy)
  #4 [general]  key-3: ••••••••••••••••••••11L0  (Healthy)
```

---

### 3.3 `orquestra vault set <id>`

Creates a new credential service or updates an existing one. Alias: `add`, `save`.

```bash
orquestra vault set <id> [OPTIONS]
```

**Options:**

- `--key <secret>`: Initial API key / secret token. Use `--key -` to read securely from standard input, avoiding shell history leakage.
- `--key-file <path>`: Read the secret from a local file.
- `--label <label>`: Human-readable label (defaults to `<id>` if omitted).
- `--preset <preset>`: Preset identifier (e.g., `openai`, `anthropic`, `github`, `cloudflare`, `aws`, `stripe`, `custom`). Defaults to `custom`.
- `--description <text>`: Brief functional description.
- `--custom-header <name>`: Custom HTTP header name (for `custom` preset).
- `--custom-prefix <prefix>`: Custom header prefix (e.g. `Bearer `, `Token `, `Key `).
- `--custom-env <var_name>`: Subprocess environment variable name (for `custom` preset).
- `--meta <KEY=VAL>`: Companion metadata (can be specified multiple times, e.g. `--meta account_id=xyz`).
- `--tag <tag>`: Optional tag for the initial key (e.g., `admin`, `workers`).
- `--append`: If the service already exists, appends the new key to its pool instead of replacing the pool.
- `--json`: Output the resulting service object as JSON.

**Example:**

```bash
# Registering a Cloudflare credential with metadata:
orquestra vault set cloudflare \
  --label "Cloudflare Production" \
  --preset cloudflare \
  --key "clfp_xxxxxxxxxxxxxxxxxxxxxxxxxxxx" \
  --meta account_id="84a91c78e3294801fa87b321a" \
  --tag "workers"

# Securely passing key from pipe without shell history contamination:
echo -n "sk-ant-api03-xxxx..." | orquestra vault set anthropic --preset anthropic --key -
```

---

### 3.4 `orquestra vault add-key <id>`

Appends a new credential key into an existing service's pool.

```bash
orquestra vault add-key <id> --key <secret> [OPTIONS]
```

**Options:**

- `--key <secret>`: Secret key string (or `-` for stdin, or use `--key-file <path>`).
- `--tag <tag>`: Functional tag identifying this key's purpose (e.g., `read-only`, `d1`, `backup`).
- `--json`: Output updated service status as JSON.

**Example:**

```bash
orquestra vault add-key openai --key "sk-proj-backupkey..." --tag "fallback"
```

---

### 3.5 `orquestra vault remove-key <id>`

Removes a specific credential from a service pool by its 1-based key number.

```bash
orquestra vault remove-key <id> --key-number <N> [OPTIONS]
```

**Options:**

- `--key-number <N>`: 1-based index of the key to remove (required).
- `--yes` / `-y`: Skip confirmation prompt.
- `--json`: Output updated service in JSON.

> [!NOTE]
> The vault refuses to delete the last remaining key of a service. To remove the entire service, use `orquestra vault remove <id>`.

---

### 3.6 `orquestra vault remove <id>`

Removes an entire credential service from the vault. Alias: `delete`.

```bash
orquestra vault remove <id> [OPTIONS]
```

**Options:**

- `--yes` / `-y`: Skip confirmation prompt.
- `--json`: Output confirmation as JSON.

**Human Output Example:**

```text
Service 'cloudflare' successfully removed from the vault.
```

---

### 3.7 `orquestra vault enable` / `disable` / `toggle` / `set-enabled`

Controls the operator kill-switch for a service. When disabled, the blind proxy immediately refuses any HTTP or exec request directed to that service, failing closed before secrets touch memory or the network.

```bash
orquestra vault enable <id> [--json]
orquestra vault disable <id> [--json]
orquestra vault toggle <id> [--json]
orquestra vault set-enabled <id> <on|off> [--json]
```

**Example:**

```bash
orquestra vault disable openai
# Output: Service 'OpenAI Production' disabled. Blind proxy calls will fail closed.
```

---

### 3.8 `orquestra vault set-primary <id>`

Designates which key in the pool is the active default (`#1` based index).

```bash
orquestra vault set-primary <id> --key-number <N> [OPTIONS]
```

**Options:**

- `--key-number <N>`: 1-based index of the key to make primary.
- `--json`: Output updated service as JSON.

**Example:**

```bash
orquestra vault set-primary openai --key-number 2
# Output: Key #2 is now the primary active key for service 'openai'.
```

---

### 3.9 `orquestra vault rotate <id>`

Advances or replaces credentials within a pool.

```bash
orquestra vault rotate <id> [OPTIONS]
```

**Options:**

- `--next`: Advances the active primary pointer to the next healthy key in the pool (round-robin).
- `--key-number <N>`: Target key index to replace.
- `--new-key <secret>`: New secret string to replace the targeted key (supports `-` for stdin).
- `--json`: Output result as JSON.

**Example:**

```bash
# Advance to the next healthy key in the pool:
orquestra vault rotate openai --next

# In-place key replacement:
orquestra vault rotate github --key-number 1 --new-key "ghp_freshnewtoken..."
```

---

### 3.10 `orquestra vault test <id>`

Tests connectivity and credential validity against the provider's canonical health endpoint without exposing secrets. Persists the observed health state (`Healthy` or `Degraded`) into the vault. Alias: `check`, `ping`.

```bash
orquestra vault test <id> [OPTIONS]
```

**Options:**

- `--key-number <N>`: Test a specific key from the pool instead of the active primary key.
- `--json`: Output result as JSON.

**Human Output Example:**

```text
Testing service 'anthropic' with key #1...
Outcome:   SUCCESS (HTTP 200)
Latency:   184ms
Health:    Healthy
Message:   Connection verified successfully.
```

---

### 3.11 `orquestra vault proxy-http` (Alias: `http`)

Executes a credentialed blind HTTP request. The backend injects authentication headers in memory, protects against SSRF via preset host allowlists, executes failover across keys on rate limits or auth errors, and scrubs all secrets from the response payload.

```bash
orquestra vault proxy-http [OPTIONS]
```

**Options:**

- `--service <id>`: Target service ID (required).
- `--endpoint <url>`: Target HTTPS endpoint (required). Host must match the preset's allowlist.
- `--method <verb>`: HTTP method (`GET`, `POST`, `PUT`, `DELETE`, etc. Default: `GET`).
- `--headers <json>`: Optional extra HTTP headers as a JSON object (e.g. `'{"Content-Type":"application/json"}'`).
- `--body <text>`: Request body text.
- `--key-tag <tag>`: Pin request to a key bearing a specific tag.
- `--key-number <N>`: Pin request to a specific 1-based key number (disables automatic failover).
- `--json`: Output complete `VaultHttpResult` envelope (`status`, `headers`, `body`, `usedKeyIndex`, `rotated`).

**Example:**

```bash
orquestra vault proxy-http \
  --service github \
  --endpoint "https://api.github.com/user" \
  --headers '{"Accept":"application/vnd.github.v3+json"}'
```

---

### 3.12 `orquestra vault proxy-exec` (Alias: `exec`)

Executes a bare allowlisted binary in a subprocess with secrets injected strictly into environment variables. Shell strings, pipes, redirections, and non-allowlisted binaries are rejected before invocation. Output streams are scrubbed.

The binary is resolved through the process `PATH` and then the login shell's `PATH` (plus `~/.local/bin`, `~/.cargo/bin`, mise/asdf shims and Homebrew), so CLIs installed by a version manager work when the app was opened from the menu. The allowlist and the per-binary subcommand policy are checked against the bare name **before** resolution; a path supplied through IPC is still refused.

```bash
orquestra vault proxy-exec <service> [--key-number N] [--] <argv...>
```

**Allowlisted Binaries:**
`aws`, `az`, `doctl`, `echo`, `gcloud`, `gh`, `kubectl`, `printf`, `stripe`, `terraform`.

**Example:**

```bash
# Execute gh CLI with vault-injected GITHUB_TOKEN:
orquestra vault proxy-exec github -- gh issue list --repo myorg/myrepo

# Execute aws CLI with vault-injected AWS credentials & meta region:
orquestra vault proxy-exec aws -- aws s3 ls
```

---

### 3.13 `orquestra vault presets`

Lists all built-in provider presets supported by OrquestraAI, including auth header strategies, environment variable mappings, and required metadata fields.

```bash
orquestra vault presets [OPTIONS]
```

**Options:**

- `--json`: Output full preset specifications in JSON.

**Human Output Example:**

```text
PRESET        LABEL            AUTH HEADER      ENV VAR               META FIELDS
openai        OpenAI           Authorization    OPENAI_API_KEY        -
anthropic     Anthropic        x-api-key        ANTHROPIC_API_KEY     -
github        GitHub           Authorization    GITHUB_TOKEN          -
cloudflare    Cloudflare       Authorization    CLOUDFLARE_API_TOKEN  account_id
supabase      Supabase         apikey + mirror  SUPABASE_KEY          project_url
aws           Amazon AWS       Custom Signature AWS_SECRET_ACCESS_KEY AWS_ACCESS_KEY_ID, AWS_DEFAULT_REGION
stripe        Stripe           Authorization    STRIPE_API_KEY        -
custom        Custom Service   (user-defined)   (user-defined)        -
```

---

### 3.14 `orquestra vault audit`

Performs a comprehensive security assessment of the local Vault subsystem.

```bash
orquestra vault audit [OPTIONS]
```

**Diagnostics Executed:**

1. **At-Rest Storage Security:** Verifies whether `vault.json` is encrypted via OS Keyring or fallback file; checks file permissions (`0600`).
2. **Plaintext Migration Backups:** Scans for lingering `vault.json.migrate-bak` or `mcps.json.migrate-bak` files left from pre-alpha.5 migrations.
3. **Key Pool Health:** Reports total keys, healthy vs degraded keys, and active cooldown countdowns.
4. **Execution Policy:** Validates allowlisted binaries and SSRF endpoint rules.

**Human Output Example:**

```text
Vault Security Audit:
  Storage Encryption:  AES-256-GCM (Linux Secret Service Keyring) [PASS]
  File Permissions:    ~/.orquestra/vault.json (0600) [PASS]
  Migration Backups:   1 lingering plaintext backup detected! [WARN]
                       File: ~/.orquestra/vault.json.migrate-bak
                       Run 'orquestra vault audit purge-backups' to zeroize.
  Pool Integrity:      4 services, 10 keys (9 Healthy, 1 Cooldown)
  Exec Policy:         Allowlist active (10 binaries), No shell escapes
```

#### Purging Migration Backups:

```bash
orquestra vault audit purge-backups --confirm [--json]
```

Overwrites with zeroes, flushes to disk (`sync`), and unlinks the `.migrate-bak` files after verifying that the active sealed vault opens successfully.

---

## 4. Exit Codes & Automation

- `0`: Success. Operation completed cleanly.
- `1`: Operation Error. Backend failure (e.g. unknown service, remote HTTP 500, binary execution failure).
- `2`: Usage Error. Missing required flags, invalid subcommand, or syntax error.

### Scripting with `jq` Example:

```bash
# Query health of all enabled services:
orquestra vault list --enabled-only --json | jq '.[] | {id: .id, health: .health, keys: .totalKeys}'

# Test all services and alert on non-healthy endpoints:
for svc in $(orquestra vault list --enabled-only --json | jq -r '.[].id'); do
  status=$(orquestra vault test "$svc" --json | jq -r '.health')
  echo "Service $svc health: $status"
done
```

---

## 5. Security & Isolation Guidelines

1. **Never Log Secrets:** CLI subcommands default to masking secrets (`••••••••`). Plaintext secrets are revealed only when the user explicitly provides `--reveal` on `inspect`.
2. **Avoid Bash History Exposure:** When automating token insertion, prefer piped stdin (`--key -`) or `--key-file` over inline CLI arguments (`--key sk-...`).
3. **SSRF Guardrails:** `proxy-http` enforces strict destination domain checks based on presets. Arbitrary internal network IPs (e.g. `169.254.169.254`, `localhost`, `10.0.0.0/8`) are rejected before transmission.
4. **No Shell Invocations:** `proxy-exec` bypasses the shell entirely by executing bare binary paths directly with `Command::args(&argv[1..])`. Shell metacharacters (`|`, `&`, `;`, `>`, `$()`) cannot be interpreted.
