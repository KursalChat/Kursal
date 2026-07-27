import { invoke } from '@tauri-apps/api/core';

export type BenchmarkMode = 'parallel' | 'sequential';

export interface BenchmarkMeta {
  id: string;
  mode: BenchmarkMode;
  default_iterations: number;
}

export interface BenchmarkProgress {
  current: number;
  total: number;
  elapsed_ms: number;
}

export interface BenchmarkResult {
  iterations: number;
  average_per_iteration_ms: number;
  average_with_threading_ms: number;
  total_ms: number;
  iterations_per_second: number;
}

export async function listBenchmarks(): Promise<BenchmarkMeta[]> {
  return invoke('list_benchmarks');
}

export async function runBenchmark(id: string, iterations: number): Promise<BenchmarkResult> {
  return invoke('run_benchmark', { id, iterations });
}

export async function cancelBenchmark(): Promise<void> {
  return invoke('cancel_benchmark');
}

export async function isBenchmarkRunning(): Promise<boolean> {
  return invoke('is_benchmark_running');
}
