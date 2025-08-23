//! Ring buffer performance benchmarks

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use veddb_core::ring::{SpscRingBuffer, MpmcRingBuffer, Slot};
use std::sync::Arc;
use std::thread;

fn bench_spsc_ring(c: &mut Criterion) {
    let mut group = c.benchmark_group("spsc_ring");
    
    for capacity in [256, 1024, 4096].iter() {
        group.bench_with_input(
            BenchmarkId::new("single_threaded", capacity),
            capacity,
            |b, &capacity| {
                let ring_buf = SpscRingBuffer::new(capacity);
                let ring = ring_buf.ring();
                
                b.iter(|| {
                    let slot = Slot::inline_data(b"test_data").unwrap();
                    ring.push(black_box(slot));
                    let _result = ring.pop();
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("producer_consumer", capacity),
            capacity,
            |b, &capacity| {
                b.iter(|| {
                    let ring_buf = Arc::new(SpscRingBuffer::new(capacity));
                    let ring_producer = ring_buf.clone();
                    let ring_consumer = ring_buf.clone();
                    
                    let producer = thread::spawn(move || {
                        for i in 0..1000 {
                            let slot = Slot::arena_offset(8, i as u64);
                            ring_producer.ring().push(slot);
                        }
                    });
                    
                    let consumer = thread::spawn(move || {
                        for _ in 0..1000 {
                            let _slot = ring_consumer.ring().pop();
                        }
                    });
                    
                    producer.join().unwrap();
                    consumer.join().unwrap();
                });
            },
        );
    }
    
    group.finish();
}

fn bench_mpmc_ring(c: &mut Criterion) {
    let mut group = c.benchmark_group("mpmc_ring");
    
    for capacity in [256, 1024, 4096].iter() {
        group.bench_with_input(
            BenchmarkId::new("single_threaded", capacity),
            capacity,
            |b, &capacity| {
                let ring_buf = MpmcRingBuffer::new(capacity);
                let ring = ring_buf.ring();
                
                b.iter(|| {
                    let slot = Slot::inline_data(b"test_data").unwrap();
                    ring.push(black_box(slot));
                    let _result = ring.pop();
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("multi_producer_consumer", capacity),
            capacity,
            |b, &capacity| {
                b.iter(|| {
                    let ring_buf = Arc::new(MpmcRingBuffer::new(capacity));
                    let mut handles = Vec::new();
                    
                    // 2 producers
                    for producer_id in 0..2 {
                        let ring = ring_buf.clone();
                        let handle = thread::spawn(move || {
                            for i in 0..500 {
                                let value = producer_id * 1000 + i;
                                let slot = Slot::arena_offset(8, value as u64);
                                ring.ring().push(slot);
                            }
                        });
                        handles.push(handle);
                    }
                    
                    // 2 consumers
                    for _ in 0..2 {
                        let ring = ring_buf.clone();
                        let handle = thread::spawn(move || {
                            for _ in 0..500 {
                                let _slot = ring.ring().pop();
                            }
                        });
                        handles.push(handle);
                    }
                    
                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }
    
    group.finish();
}

criterion_group!(benches, bench_spsc_ring, bench_mpmc_ring);
criterion_main!(benches);
