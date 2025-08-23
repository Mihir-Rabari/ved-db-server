//! KV store performance benchmarks

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use veddb_core::{VedDb, VedDbConfig};
use std::sync::Arc;
use std::thread;

fn bench_kv_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("kv_operations");
    
    // Create VedDB instance for benchmarks
    let config = VedDbConfig {
        memory_size: 64 * 1024 * 1024, // 64MB
        ..Default::default()
    };
    let veddb = Arc::new(VedDb::create("bench_kv", config).unwrap());
    
    // Benchmark SET operations
    group.bench_function("set_small", |b| {
        let key = b"benchmark_key";
        let value = b"small_value";
        
        b.iter(|| {
            let cmd = veddb_core::Command::set(1, key.to_vec(), value.to_vec());
            let _response = veddb.process_command(black_box(cmd));
        });
    });
    
    group.bench_function("set_large", |b| {
        let key = b"benchmark_key_large";
        let value = vec![0u8; 4096]; // 4KB value
        
        b.iter(|| {
            let cmd = veddb_core::Command::set(1, key.to_vec(), value.clone());
            let _response = veddb.process_command(black_box(cmd));
        });
    });
    
    // Benchmark GET operations (after setting up data)
    for i in 0..1000 {
        let key = format!("get_bench_key_{}", i);
        let value = format!("value_{}", i);
        let cmd = veddb_core::Command::set(1, key.as_bytes().to_vec(), value.as_bytes().to_vec());
        veddb.process_command(cmd);
    }
    
    group.bench_function("get_existing", |b| {
        b.iter(|| {
            let key_idx = black_box(42); // Always get the same key for consistency
            let key = format!("get_bench_key_{}", key_idx);
            let cmd = veddb_core::Command::get(1, key.as_bytes().to_vec());
            let _response = veddb.process_command(cmd);
        });
    });
    
    group.bench_function("get_missing", |b| {
        let key = b"nonexistent_key";
        
        b.iter(|| {
            let cmd = veddb_core::Command::get(1, key.to_vec());
            let _response = veddb.process_command(black_box(cmd));
        });
    });
    
    // Benchmark CAS operations
    group.bench_function("cas_success", |b| {
        // Set up a key for CAS
        let key = b"cas_bench_key";
        let initial_value = b"initial";
        let cmd = veddb_core::Command::set(1, key.to_vec(), initial_value.to_vec());
        veddb.process_command(cmd);
        
        b.iter(|| {
            let new_value = b"updated";
            let cmd = veddb_core::Command::cas(1, key.to_vec(), new_value.to_vec(), 1);
            let _response = veddb.process_command(black_box(cmd));
            
            // Reset for next iteration
            let reset_cmd = veddb_core::Command::set(1, key.to_vec(), initial_value.to_vec());
            veddb.process_command(reset_cmd);
        });
    });
    
    group.finish();
}

fn bench_concurrent_kv(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_kv");
    
    let config = VedDbConfig {
        memory_size: 128 * 1024 * 1024, // 128MB
        ..Default::default()
    };
    let veddb = Arc::new(VedDb::create("bench_concurrent", config).unwrap());
    
    group.bench_function("concurrent_sets", |b| {
        b.iter(|| {
            let veddb_clone = veddb.clone();
            let mut handles = Vec::new();
            
            // Spawn 4 threads doing SET operations
            for thread_id in 0..4 {
                let veddb = veddb_clone.clone();
                let handle = thread::spawn(move || {
                    for i in 0..100 {
                        let key = format!("thread_{}_key_{}", thread_id, i);
                        let value = format!("value_{}", i);
                        let cmd = veddb_core::Command::set(
                            1, 
                            key.as_bytes().to_vec(), 
                            value.as_bytes().to_vec()
                        );
                        veddb.process_command(cmd);
                    }
                });
                handles.push(handle);
            }
            
            for handle in handles {
                handle.join().unwrap();
            }
        });
    });
    
    group.bench_function("mixed_workload", |b| {
        // Pre-populate some data
        for i in 0..1000 {
            let key = format!("mixed_key_{}", i);
            let value = format!("value_{}", i);
            let cmd = veddb_core::Command::set(1, key.as_bytes().to_vec(), value.as_bytes().to_vec());
            veddb.process_command(cmd);
        }
        
        b.iter(|| {
            let veddb_clone = veddb.clone();
            let mut handles = Vec::new();
            
            // Spawn threads with mixed workload (70% GET, 30% SET)
            for thread_id in 0..4 {
                let veddb = veddb_clone.clone();
                let handle = thread::spawn(move || {
                    for i in 0..100 {
                        if i % 10 < 7 {
                            // GET operation
                            let key_idx = (thread_id * 100 + i) % 1000;
                            let key = format!("mixed_key_{}", key_idx);
                            let cmd = veddb_core::Command::get(1, key.as_bytes().to_vec());
                            veddb.process_command(cmd);
                        } else {
                            // SET operation
                            let key = format!("new_thread_{}_key_{}", thread_id, i);
                            let value = format!("new_value_{}", i);
                            let cmd = veddb_core::Command::set(
                                1,
                                key.as_bytes().to_vec(),
                                value.as_bytes().to_vec()
                            );
                            veddb.process_command(cmd);
                        }
                    }
                });
                handles.push(handle);
            }
            
            for handle in handles {
                handle.join().unwrap();
            }
        });
    });
    
    group.finish();
}

criterion_group!(benches, bench_kv_operations, bench_concurrent_kv);
criterion_main!(benches);
