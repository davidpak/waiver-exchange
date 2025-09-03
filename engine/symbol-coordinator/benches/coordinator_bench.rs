use criterion::{Criterion, black_box, criterion_group, criterion_main};
use symbol_coordinator::{CoordinatorConfig, SymbolCoordinator, SymbolCoordinatorApi};

fn bench_coordinator_creation(c: &mut Criterion) {
    c.bench_function("coordinator_creation", |b| {
        b.iter(|| {
            let config = CoordinatorConfig::default();
            black_box(SymbolCoordinator::new(config));
        });
    });
}

fn bench_ensure_active_placeholder(c: &mut Criterion) {
    let config = CoordinatorConfig::default();
    let coordinator = SymbolCoordinator::new(config);

    c.bench_function("ensure_active_placeholder", |b| {
        b.iter(|| {
            black_box(coordinator.ensure_active(1)).unwrap();
        });
    });
}

criterion_group!(benches, bench_coordinator_creation, bench_ensure_active_placeholder);
criterion_main!(benches);
