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

export const listBenchmarks = (): Promise<BenchmarkMeta[]> => invoke('list_benchmarks');

export const runBenchmark = (id: string, iterations: number): Promise<BenchmarkResult> =>
  invoke('run_benchmark', { id, iterations });

export const cancelBenchmark = (): Promise<void> => invoke('cancel_benchmark');

export const isBenchmarkRunning = (): Promise<boolean> => invoke('is_benchmark_running');
