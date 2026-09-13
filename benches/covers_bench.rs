// Benchmark comparativo de la caché LRU de carátulas.
//
// Enfrenta la implementación manual (`HashMap` + `VecDeque`) contra `lru::LruCache`
// sobre las mismas operaciones (acierto, fallo, inserción, desalojo y ciclo completo)
// a las dos capacidades configuradas (64 normal y 16 low-resource). Cada operación
// mide la ruta completa, incluyendo el `Mutex` que envuelve a la caché real.
//
// Ejecutar: cargo bench --bench covers_bench

use criterion::{
    BatchSize, BenchmarkGroup, Criterion, Throughput, criterion_group, criterion_main,
};
use iced::widget::image::Handle;
use lru::LruCache;
use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::hint::black_box;
use std::num::NonZeroUsize;

/// Claves de trabajo precalculadas fuera de la rutina medida. Se generan más que
/// la capacidad para poder insertar una clave nueva y forzar un desalojo.
const EXTRA_KEYS: usize = 8;

/// Operaciones que ambas implementaciones exponen para medirlas con el mismo arnés.
trait CacheOps {
    fn get_hit(&mut self, key: &str) -> Option<Handle>;
    fn get_miss(&mut self, key: &str) -> Option<Handle>;
    fn insert(&mut self, key: String, value: Handle);
    fn evict_lru(&mut self) -> Option<Handle>;
    fn len(&self) -> usize;
}

/// Réplica de la caché manual de producción: mapa de ruta a `Handle`, cola con el
/// orden de acceso (frente = menos reciente) y límite dinámico de entradas.
struct HandRolled {
    map: HashMap<String, Handle>,
    order: VecDeque<String>,
    cap: usize,
}

impl HandRolled {
    fn new(cap: usize) -> Self {
        Self {
            map: HashMap::with_capacity(cap),
            order: VecDeque::with_capacity(cap),
            cap,
        }
    }
}

impl CacheOps for HandRolled {
    fn get_hit(&mut self, key: &str) -> Option<Handle> {
        let handle = self.map.get(key).cloned()?;
        // Actualiza el orden: saca la clave de su posición y la mueve al final.
        // Este recorrido lineal es la ruta O(n) que motiva la comparación.
        if let Some(idx) = self.order.iter().position(|x| x == key) {
            self.order.remove(idx);
            self.order.push_back(key.to_string());
        }
        Some(handle)
    }

    fn get_miss(&mut self, key: &str) -> Option<Handle> {
        self.map.get(key).cloned()
    }

    fn insert(&mut self, key: String, value: Handle) {
        self.map.insert(key.clone(), value);
        self.order.push_back(key);
        while self.order.len() > self.cap {
            if let Some(oldest) = self.order.pop_front() {
                self.map.remove(&oldest);
            }
        }
    }

    fn evict_lru(&mut self) -> Option<Handle> {
        let key = self.order.pop_front()?;
        self.map.remove(&key)
    }

    fn len(&self) -> usize {
        self.order.len()
    }
}

/// Adaptador sobre `lru::LruCache`; `get` promueve a MRU y `put` desaloja solo.
struct LruAdapter {
    cache: LruCache<String, Handle>,
}

impl LruAdapter {
    fn new(cap: usize) -> Self {
        Self {
            cache: LruCache::new(
                NonZeroUsize::new(cap).expect("la capacidad debe ser mayor que cero"),
            ),
        }
    }
}

impl CacheOps for LruAdapter {
    fn get_hit(&mut self, key: &str) -> Option<Handle> {
        // `get` (no `peek`) promueve el acierto a la posición más reciente.
        self.cache.get(key).cloned()
    }

    fn get_miss(&mut self, key: &str) -> Option<Handle> {
        self.cache.get(key).cloned()
    }

    fn insert(&mut self, key: String, value: Handle) {
        self.cache.put(key, value);
    }

    fn evict_lru(&mut self) -> Option<Handle> {
        self.cache.pop_lru().map(|(_, value)| value)
    }

    fn len(&self) -> usize {
        self.cache.len()
    }
}

/// Claves sintéticas: no hay E/S de disco en la ruta medida.
fn keys_for(cap: usize) -> Vec<String> {
    (0..cap + EXTRA_KEYS)
        .map(|i| format!("/music/album/{i}"))
        .collect()
}

/// `Handle::from_path` es barato y produce un `Id` determinista y comparable;
/// a diferencia de `from_bytes`, no copia píxeles ni genera identidades únicas.
fn handle_for(i: usize) -> Handle {
    Handle::from_path(format!("cache/covers/{i}.avif"))
}

/// Deja la caché con las `cap` primeras claves, en el mismo orden que la producción
/// (la primera insertada queda como la menos reciente).
fn populate<C: CacheOps>(cache: &mut C, keys: &[String], cap: usize) {
    for (i, key) in keys.iter().take(cap).enumerate() {
        cache.insert(key.clone(), handle_for(i));
    }
    debug_assert_eq!(cache.len(), cap.min(keys.len()));
}

/// Ciclo de trabajo completo: recorre todas las claves en orden inverso al de
/// recencia (cada acierto las promueve), inserta una clave nueva para forzar un
/// desalojo y retira la menos reciente para volver al estado estable.
fn full_cycle<C: CacheOps>(cache: &mut C, all_keys: &[String], new_key: &str, value: &Handle) {
    for key in all_keys.iter().rev() {
        black_box(cache.get_hit(black_box(key)));
    }
    cache.insert(new_key.to_string(), value.clone());
    black_box(cache.evict_lru());
}

/// Registra las cinco operaciones de una implementación en un grupo de capacidad.
///
/// El `setup` no se mide: construye una caché pre-poblada nueva por lote, de modo que
/// cada llamada a la rutina parta del mismo estado y el costo sea constante.
fn bench_ops<C, F>(
    group: &mut BenchmarkGroup<'_, criterion::measurement::WallTime>,
    label: &str,
    cap: usize,
    keys: &[String],
    make: F,
) where
    C: CacheOps,
    F: Fn() -> C + Copy,
{
    let all_keys = keys[..cap].to_vec();
    let hit_key = keys[0].clone();
    let miss_key = format!("/music/album/absent-{cap}");
    let insert_key = keys[cap].clone();
    let insert_handle = handle_for(cap);

    let fresh = move || {
        let mut cache = make();
        populate(&mut cache, keys, cap);
        Mutex::new(cache)
    };

    // Cada operación suelta cuenta un elemento; el ciclo completo, `cap + 2`
    // (un acierto por clave, una inserción y un desalojo).
    group.throughput(Throughput::Elements(1));

    group.bench_function(format!("{label}/get_hit"), |b| {
        b.iter_batched_ref(
            fresh,
            |mutex| {
                let mut cache = mutex.lock();
                black_box(cache.get_hit(black_box(&hit_key)));
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function(format!("{label}/get_miss"), |b| {
        b.iter_batched_ref(
            fresh,
            |mutex| {
                let mut cache = mutex.lock();
                black_box(cache.get_miss(black_box(&miss_key)));
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function(format!("{label}/insert"), |b| {
        let key = insert_key.clone();
        let value = insert_handle.clone();
        b.iter_batched_ref(
            fresh,
            move |mutex| {
                let mut cache = mutex.lock();
                cache.insert(key.clone(), value.clone());
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function(format!("{label}/evict"), |b| {
        b.iter_batched_ref(
            fresh,
            |mutex| {
                let mut cache = mutex.lock();
                black_box(cache.evict_lru());
            },
            BatchSize::SmallInput,
        );
    });

    group.throughput(Throughput::Elements((cap + 2) as u64));
    group.bench_function(format!("{label}/full_cycle"), |b| {
        let new_key = insert_key.clone();
        let value = insert_handle.clone();
        b.iter_batched_ref(
            fresh,
            |mutex| {
                let mut cache = mutex.lock();
                full_cycle(&mut *cache, &all_keys, &new_key, &value);
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_covercache(c: &mut Criterion) {
    for cap in [64usize, 16] {
        let keys = keys_for(cap);
        let mut group = c.benchmark_group(format!("covercache/{cap}"));
        bench_ops(&mut group, "hand_rolled", cap, &keys, || {
            HandRolled::new(cap)
        });
        bench_ops(&mut group, "lru", cap, &keys, || LruAdapter::new(cap));
        group.finish();
    }
}

criterion_group!(
    name = covers_grp;
    config = Criterion::default().sample_size(50);
    targets = bench_covercache
);
criterion_main!(covers_grp);
