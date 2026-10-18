import { invoke } from "@tauri-apps/api/core";

export interface MemoryStats {
  totalMb: number;
  usedMb: number;
  freeMb: number;
  usedPercent: number;
}

export interface TmpStats extends MemoryStats {
  path: string;
  isTmpfs: boolean;
}

export interface CpuStats {
  cores: number;
  loadPercent: number;
}

export interface TopProcess {
  pid: number;
  name: string;
  rssMb: number;
}

export interface SystemHealth {
  ram: MemoryStats;
  swap: MemoryStats;
  tmp: TmpStats;
  cpu: CpuStats;
  topProcesses: TopProcess[];
  alert: boolean;
}

export interface SweepReport {
  reclaimedMb: number;
  removedCount: number;
  status: string;
}

export function getSystemHealth(): Promise<SystemHealth> {
  return invoke<SystemHealth>("get_system_health");
}

export function sweepSystemCaches(): Promise<SweepReport> {
  return invoke<SweepReport>("sweep_system_caches");
}
