#!/usr/bin/env python3
"""Live gate for Phase 5.

Runs the real binary against the real harness configs, and after every step
re-hashes everything *except* the MCP section, so a write that clobbers
`numStartups`, `projects`, `skills`, `permission`, model settings or Codex
tables fails loudly.
"""

import hashlib
import json
import pathlib
import subprocess
import sys
import tomllib

HOME = pathlib.Path.home()
BIN = HOME / "Persistente/Coding/OrquestraAI/src-tauri/target/debug/orquestra-ai"
SNAP = pathlib.Path("/tmp/mcp-live-before")

JSON_TARGETS = {
    "claude": (".claude.json", "mcpServers"),
    "antigravity": (".gemini/config/mcp_config.json", "mcpServers"),
    "copilot": (".copilot/mcp-config.json", "mcpServers"),
    "opencode": (".config/opencode/opencode.jsonc", "mcp"),
    "kilo": (".config/kilo/kilo.jsonc", "mcp"),
}
TOML_TARGETS = {"codex": (".codex/config.toml", "mcp_servers")}

failures: list[str] = []
checks = 0


def check(label: str, ok: bool, detail: str = "") -> None:
    global checks
    checks += 1
    print(f"{'PASS' if ok else 'FAIL'}  {label}{f'  — {detail}' if detail else ''}")
    if not ok:
        failures.append(label)


def load_json(rel: str) -> dict:
    return json.loads((HOME / rel).read_text())


def load_toml(rel: str) -> dict:
    return tomllib.loads((HOME / rel).read_text())


def non_mcp_digest(target: str) -> str:
    if target in JSON_TARGETS:
        rel, key = JSON_TARGETS[target]
        doc = load_json(rel)
        doc.pop(key, None)
    else:
        rel, key = TOML_TARGETS[target]
        doc = load_toml(rel)
        doc.pop(key, None)
    return hashlib.sha256(
        json.dumps(doc, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()[:16]


def baseline_digest(target: str) -> str:
    """Same digest computed from the untouched pre-run snapshot."""
    if target in JSON_TARGETS:
        rel, key = JSON_TARGETS[target]
        doc = json.loads((SNAP / rel).read_text())
        doc.pop(key, None)
    else:
        rel, key = TOML_TARGETS[target]
        doc = tomllib.loads((SNAP / rel).read_text())
        doc.pop(key, None)
    return hashlib.sha256(
        json.dumps(doc, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()[:16]


def keys_for(target: str) -> list[str]:
    if target in JSON_TARGETS:
        rel, key = JSON_TARGETS[target]
        return sorted(load_json(rel).get(key, {}).keys())
    rel, key = TOML_TARGETS[target]
    return sorted(load_toml(rel).get(key, {}).keys())


def contents(target: str) -> dict:
    if target in JSON_TARGETS:
        rel, key = JSON_TARGETS[target]
        return load_json(rel).get(key, {})
    rel, key = TOML_TARGETS[target]
    return load_toml(rel).get(key, {})


def run(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run([str(BIN), *args], capture_output=True, text=True)


def step(label: str, *args: str) -> subprocess.CompletedProcess:
    result = run(*args)
    check(
        label,
        result.returncode == 0,
        (result.stdout or result.stderr).strip().splitlines()[0]
        if (result.stdout or result.stderr).strip()
        else "",
    )
    return result


def assert_preserved(stage: str) -> None:
    for target in [*JSON_TARGETS, *TOML_TARGETS]:
        expected = baseline_digest(target)
        actual = non_mcp_digest(target)
        check(
            f"{stage}: non-MCP content intact in {target}",
            expected == actual,
            f"{expected} vs {actual}",
        )


def main() -> int:
    print("== 0. compile catalog into every harness ==")
    step("deploy all", "--mcp-deploy")
    assert_preserved("deploy")
    for target in JSON_TARGETS | TOML_TARGETS:
        keys = keys_for(target)
        missing = [s for s in ("canva", "github", "headroom", "memory", "papinho", "puppeteer", "rea") if s not in keys]
        check(f"deploy: {target} lists all 7 catalog servers", not missing, f"{len(keys)} keys, missing={missing}")

    before_codex = tomllib.loads((SNAP / ".codex/config.toml").read_text())["mcp_servers"]
    check(
        "deploy: codex keeps a foreign server OrquestraAI does not own",
        before_codex.get("node_repl") is not None
        and contents("codex").get("node_repl") == before_codex["node_repl"],
        str(sorted(contents("codex"))),
    )

    codex = contents("codex")
    check(
        "deploy: codex canva table carries command + args",
        codex.get("canva", {}).get("command") == "npx"
        and "mcp" in codex.get("canva", {}).get("args", []),
        str(codex.get("canva")),
    )
    claude = contents("claude")
    github_fields = claude.get("github", {})
    check(
        "deploy: claude entry is {command,args,env} (env only when declared)",
        set(github_fields) == {"command", "args", "env"}
        and set(claude.get("canva", {})) == {"command", "args"}
        and "GITHUB_PERSONAL_ACCESS_TOKEN" in github_fields.get("env", {}),
        f"github={sorted(github_fields)} canva={sorted(claude.get('canva', {}))}",
    )
    opencode = contents("opencode")
    rea_entry = opencode.get("rea", {})
    check(
        "deploy: opencode uses mcp + merged command array + environment + type",
        opencode.get("canva", {}).get("type") == "local"
        and opencode.get("canva", {}).get("enabled") is True
        and isinstance(opencode.get("canva", {}).get("command"), list)
        and "env" not in opencode.get("canva", {})
        and opencode.get("canva", {}).get("command")
        == ["npx", "-y", "@canva/cli@latest", "mcp"]
        and "JAVA_HOME" in rea_entry.get("environment", {})
        and "env" not in rea_entry,
        json.dumps(rea_entry)[:140],
    )
    paper = contents("copilot")
    check(
        "deploy: copilot keeps its pre-existing tools allowlist",
        "tools" in paper.get("rea", {}),
        str(sorted(paper.get("rea", {}))),
    )

    print("\n== 1. global off removes it everywhere ==")
    step("global off", "--mcp-set", "canva", "global", "off")
    assert_preserved("global off")
    for target in JSON_TARGETS | TOML_TARGETS:
        check(f"global off: canva gone from {target}", "canva" not in keys_for(target))

    print("\n== 2. per-harness on lands only in that harness ==")
    step("claude on", "--mcp-set", "canva", "claude", "on")
    assert_preserved("claude on")
    check("claude on: present in claude", "canva" in keys_for("claude"))
    for target in ["antigravity", "copilot", "opencode", "kilo", "codex"]:
        check(f"claude on: absent from {target}", "canva" not in keys_for(target))

    print("\n== 3. inheritance lock refuses without touching a file ==")
    before = {t: hashlib.sha256(pathlib.Path(HOME / rel).read_bytes()).hexdigest()
              for t, (rel, _) in {**JSON_TARGETS, **TOML_TARGETS}.items()}
    locked = run("--mcp-set", "github", "codex", "off")
    after = {t: hashlib.sha256(pathlib.Path(HOME / rel).read_bytes()).hexdigest()
             for t, (rel, _) in {**JSON_TARGETS, **TOML_TARGETS}.items()}
    check(
        "lock: inherited server refuses a per-harness disable",
        locked.returncode == 1 and "inherited from Global" in locked.stderr,
        locked.stderr.strip(),
    )
    check("lock: refusal wrote nothing", before == after)

    print("\n== 4. per-harness off removes only that harness ==")
    step("claude off", "--mcp-set", "canva", "claude", "off")
    assert_preserved("claude off")
    check("claude off: canva gone from claude", "canva" not in keys_for("claude"))

    print("\n== 5. global on restores the baseline ==")
    step("global on", "--mcp-set", "canva", "global", "on")
    assert_preserved("global on")
    for target in JSON_TARGETS | TOML_TARGETS:
        check(f"global on: canva back in {target}", "canva" in keys_for(target))

    print("\n== 6. idempotent re-deploy writes nothing ==")
    result = step("second deploy", "--mcp-deploy")
    check(
        "idempotent: no config changed on re-deploy",
        json.loads(result.stdout)["changed"] == [],
        result.stdout.strip(),
    )

    print(f"\n{checks - len(failures)}/{checks} checks passed")
    if failures:
        print("FAILED: " + "; ".join(failures))
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
