/** System health: live metrics and safe temp-file cleanup. */
export const janitor = {
  title: "Saúde do sistema",
  subtitle: "Métricas ao vivo e limpeza segura de arquivos temporários.",
  refresh: "Atualizar métricas",
  alert: "Uso de memória ou de /tmp acima do limite recomendado.",
  ram: "Memória RAM",
  swap: "Memória swap",
  tmp: "/tmp",
  cpu: "Carga da CPU",
  cores: "núcleos",
  mb: "MB",
  topProcesses: "Maiores processos por RAM",
  tmpPath: (path: string, isTmpfs: boolean) => `${path}${isTmpfs ? " · tmpfs" : ""}`,
  noProcesses: "Nenhum processo visível.",
  loading: "Lendo métricas do sistema…",
  unavailable: "Métricas indisponíveis.",
  sweepHint: "Remove apenas sockets inativos e scratch com mais de 6 horas.",
  sweep: "Limpar caches antigos",
  sweeping: "Limpando…",
  sweepComplete: (count: number, mb: number) =>
    `${count} arquivo(s) removido(s) · ${mb.toFixed(1)} MB liberados`,
  ramAlert: (percent: number, threshold: number) =>
    `RAM em ${percent}% — acima do limite de ${threshold}%`,
  sweepFailed: (detail: string) => `Falha ao limpar caches: ${detail}`,
  readFailed: (detail: string) => `Falha ao ler métricas: ${detail}`,
};
