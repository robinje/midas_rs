//! Rust implementation of
//! [https://github.com/bhatiasiddharth/MIDAS](https://github.com/bhatiasiddharth/MIDAS)
//!
//! ```rust
//! use midas_rs::{Int, Float, MidasR};
//!
//! fn main() {
//!     // For configuration options, refer to MidasRParams
//!     let mut midas = MidasR::new(Default::default());
//!
//!     println!("{:.6}", midas.insert((1, 1, 1)));
//!     println!("{:.6}", midas.insert((1, 2, 1)));
//!     println!("{:.6}", midas.insert((1, 1, 2)));
//!     println!("{:.6}", midas.insert((1, 2, 3)));
//!
//!     assert_eq!(midas.insert((1, 2, 4)), midas.query(1, 2));
//! }
//! ```

use rand::rngs::SmallRng;

pub mod default {
    use super::{Float, Int};

    pub const NUM_ROWS: Int = 2;
    pub const NUM_BUCKETS: Int = 769;
    pub const M_VALUE: Int = 773;
    pub const ALPHA: Float = 0.6;
}

pub type Int = u64;
pub type Float = f64;
const FLOAT_MAX: Float = std::f64::MAX;

struct Row {
    a: Int,
    b: Int,
    buckets: Vec<Float>,
}

impl Row {
    fn new(buckets: Int, rng: &mut Rng) -> Self {
        Self {
            a: (rng.rand() % (buckets - 1)) + 1,
            b: rng.rand() % buckets,
            buckets: vec![0.; buckets as usize],
        }
    }

    fn hash(&self, m_value: Int, source: Int, dest: Int) -> Int {
        #![allow(unused_comparisons)]

        let resid = m_value
            .wrapping_mul(dest)
            .wrapping_add(source)
            .wrapping_mul(self.a)
            .wrapping_add(self.b)
            % self.num_buckets() as Int;

        resid
            + if resid < 0 {
                self.num_buckets() as Int
            } else {
                0
            }
    }

    fn node_insert(&mut self, a: Int, weight: Float) {
        self.insert(0, a, 0, weight)
    }

    fn insert(&mut self, m_value: Int, source: Int, dest: Int, weight: Float) {
        let hash = self.hash(m_value, source, dest) as usize;
        self.buckets[hash] += weight;
    }

    fn node_count(&self, source: Int) -> Float {
        self.count(0, source, 0)
    }

    fn count(&self, m_value: Int, source: Int, dest: Int) -> Float {
        self.buckets[self.hash(m_value, source, dest) as usize]
    }

    fn clear(&mut self) {
        for bucket in self.buckets.iter_mut() {
            *bucket = 0.;
        }
    }

    fn num_buckets(&self) -> usize {
        self.buckets.len()
    }

    fn lower(&mut self, alpha: Float) {
        for bucket in self.buckets.iter_mut() {
            *bucket = *bucket * alpha;
        }
    }
}

struct Rng(SmallRng);

impl Rng {
    fn new(seed: Int) -> Self {
        use rand::SeedableRng;
        Self(SmallRng::seed_from_u64(seed as u64))
    }

    fn rand(&mut self) -> Int {
        use rand::RngCore;
        self.0.next_u32() as Int
    }
}

struct EdgeHash {
    m_value: Int,
    rows: Vec<Row>,
}

impl EdgeHash {
    fn new(rows: Int, buckets: Int, m_value: Int, seed: Int) -> Self {
        let mut rng = Rng::new(seed);

        Self {
            m_value,
            rows: (0..rows).map(|_| Row::new(buckets, &mut rng)).collect(),
        }
    }

    fn lower(&mut self, alpha: Float) {
        for row in self.rows.iter_mut() {
            row.lower(alpha);
        }
    }

    fn clear(&mut self) {
        for row in self.rows.iter_mut() {
            row.clear();
        }
    }

    fn insert(&mut self, source: Int, dest: Int, weight: Float) {
        for row in self.rows.iter_mut() {
            row.insert(self.m_value, source, dest, weight);
        }
    }

    fn count(&self, source: Int, dest: Int) -> Float {
        self.rows
            .iter()
            .map(|row| row.count(self.m_value, source, dest))
            .fold(FLOAT_MAX, float_min)
    }
}

struct NodeHash {
    rows: Vec<Row>,
}

impl NodeHash {
    fn new(rows: Int, buckets: Int, seed: Int) -> Self {
        let mut rng = Rng::new(seed);

        Self {
            rows: (0..rows).map(|_| Row::new(buckets, &mut rng)).collect(),
        }
    }

    fn count(&self, source: Int) -> Float {
        self.rows
            .iter()
            .map(|row| row.node_count(source))
            .fold(FLOAT_MAX, float_min)
    }

    fn lower(&mut self, alpha: Float) {
        for row in self.rows.iter_mut() {
            row.lower(alpha);
        }
    }

    fn insert(&mut self, source: Int, weight: Float) {
        for row in self.rows.iter_mut() {
            row.node_insert(source, weight);
        }
    }
}

fn float_max(a: Float, b: Float) -> Float {
    if a >= b {
        a
    } else {
        b
    }
}

fn float_min(a: Float, b: Float) -> Float {
    if a <= b {
        a
    } else {
        b
    }
}

fn counts_to_anom(total: Float, current: Float, current_time: Int) -> Float {
    let current_mean = total / current_time as Float;
    let sqerr = float_max(0., current - current_mean).powi(2);
    (sqerr / current_mean) + (sqerr / (current_mean * float_max(1., (current_time - 1) as Float)))
}

pub struct MidasRParams {
    /// Number of rows of buckets to use for internal Count-Min Sketches
    pub rows: Int,
    /// Number of buckets in each rows to use for internal Count-Min Sketches
    pub buckets: Int,
    /// Value used internally in determining bucket placement. Might be
    /// made private in future version.
    pub m_value: Int,
    /// Factor used to to decay current values when our inputs signal
    /// that time has ticked ahead.
    pub alpha: Float,
}

impl Default for MidasRParams {
    fn default() -> Self {
        Self {
            rows: default::NUM_ROWS,
            buckets: default::NUM_BUCKETS,
            m_value: default::M_VALUE,
            alpha: default::ALPHA,
        }
    }
}

pub struct MidasR {
    current_time: Int,
    alpha: Float,

    current_count: EdgeHash,
    total_count: EdgeHash,

    source_score: NodeHash,
    dest_score: NodeHash,
    source_total: NodeHash,
    dest_total: NodeHash,
}

impl MidasR {
    pub fn new(
        MidasRParams {
            rows,
            buckets,
            m_value,
            alpha,
        }: MidasRParams,
    ) -> Self {
        let dumb_seed = 538;

        Self {
            current_time: 0,
            alpha,

            current_count: EdgeHash::new(rows, buckets, m_value, dumb_seed + 1),
            total_count: EdgeHash::new(rows, buckets, m_value, dumb_seed + 2),

            source_score: NodeHash::new(rows, buckets, dumb_seed + 3),
            dest_score: NodeHash::new(rows, buckets, dumb_seed + 4),
            source_total: NodeHash::new(rows, buckets, dumb_seed + 5),
            dest_total: NodeHash::new(rows, buckets, dumb_seed + 6),
        }
    }

    pub fn current_time(&self) -> Int {
        self.current_time
    }

    /// Factor used to to decay current values when our inputs signal
    /// that time has ticked ahead.
    pub fn alpha(&self) -> Float {
        self.alpha
    }

    /// # Panics
    ///
    /// If `time < self.current_time()`
    pub fn insert(&mut self, (source, dest, time): (Int, Int, Int)) -> Float {
        assert!(self.current_time <= time);

        if time > self.current_time {
            // This deviation from the original C++ implementation is
            // mentioned at
            // https://github.com/bhatiasiddharth/MIDAS/issues/7#issuecomment-597185695
            let time_delta = time - self.current_time;
            let total_decay = self.alpha.powi(time_delta as _);
            self.current_count.lower(total_decay);
            self.source_score.lower(total_decay);
            self.dest_score.lower(total_decay);

            self.current_time = time;
        }

        self.current_count.insert(source, dest, 1.);
        self.total_count.insert(source, dest, 1.);

        self.source_score.insert(source, 1.);
        self.dest_score.insert(dest, 1.);
        self.source_total.insert(source, 1.);
        self.dest_total.insert(dest, 1.);

        self.query(source, dest)
    }

    pub fn query(&self, source: Int, dest: Int) -> Float {
        let current_score = counts_to_anom(
            self.total_count.count(source, dest),
            self.current_count.count(source, dest),
            self.current_time,
        );
        let current_score_source = counts_to_anom(
            self.source_total.count(source),
            self.source_score.count(source),
            self.current_time,
        );
        let current_score_dest = counts_to_anom(
            self.dest_total.count(dest),
            self.dest_score.count(dest),
            self.current_time,
        );

        float_max(
            float_max(current_score_source, current_score_dest),
            current_score,
        )
        .ln_1p()
    }

    /// Takes an iterator of `(source, dest, time)` thruples and returns
    /// an iterator of corresponding scores.
    ///
    /// For a more ergonomic version, see `MidasIterator::midas_r`.
    ///
    /// # Panics
    ///
    /// Subsequent iterator will panic if ever passed a thruple where
    /// the third element (the time) decreases from its predecessor.
    pub fn iterate(
        data: impl Iterator<Item = (Int, Int, Int)>,
        params: MidasRParams,
    ) -> impl Iterator<Item = Float> {
        let mut midas = Self::new(params);

        data.map(move |datum| midas.insert(datum))
    }
}

pub struct MidasParams {
    /// Number of rows of buckets to use for internal Count-Min Sketches
    pub rows: Int,
    /// Number of buckets in each rows to use for internal Count-Min Sketches
    pub buckets: Int,
    /// Value used internally in determining bucket placement. Might be
    /// made private in future version.
    pub m_value: Int,
}

impl Default for MidasParams {
    fn default() -> Self {
        Self {
            rows: default::NUM_ROWS,
            buckets: default::NUM_BUCKETS,
            m_value: default::M_VALUE,
        }
    }
}

pub struct Midas {
    current_time: Int,
    current_count: EdgeHash,
    total_count: EdgeHash,
}

impl Midas {
    pub fn new(
        MidasParams {
            rows,
            buckets,
            m_value,
        }: MidasParams,
    ) -> Self {
        let dumb_seed = 39;

        Self {
            current_time: 0,
            current_count: EdgeHash::new(rows, buckets, m_value, dumb_seed + 1),
            total_count: EdgeHash::new(rows, buckets, m_value, dumb_seed + 2),
        }
    }

    pub fn current_time(&self) -> Int {
        self.current_time
    }

    /// # Panics
    ///
    /// If `time < self.current_time()`
    pub fn insert(&mut self, (source, dest, time): (Int, Int, Int)) -> Float {
        assert!(self.current_time <= time);

        if time > self.current_time {
            self.current_count.clear();
            self.current_time = time;
        }

        self.current_count.insert(source, dest, 1.);
        self.total_count.insert(source, dest, 1.);

        self.query(source, dest)
    }

    pub fn query(&self, source: Int, dest: Int) -> Float {
        let current_mean = self.total_count.count(source, dest) / self.current_time as Float;
        let sqerr = (self.current_count.count(source, dest) - current_mean).powi(2);

        if self.current_time == 1 {
            0.
        } else {
            (sqerr / current_mean) + (sqerr / (current_mean * (self.current_time - 1) as Float))
        }
    }

    /// Takes an iterator of `(source, dest, time)` thruples and returns
    /// an iterator of corresponding scores.
    ///
    /// For a more ergonomic version, see `MidasIterator::midas`.
    ///
    /// # Panics
    ///
    /// Subsequent iterator will panic if ever passed a thruple where
    /// the third element (the time) decreases from its predecessor.
    pub fn iterate(
        data: impl Iterator<Item = (Int, Int, Int)>,
        params: MidasParams,
    ) -> impl Iterator<Item = Float> {
        let mut midas = Self::new(params);

        data.map(move |datum| midas.insert(datum))
    }
}

pub trait MidasIterator<'a>: 'a + Sized + Iterator<Item = (Int, Int, Int)> {
    /// Takes an iterator of `(source, dest, time)` thruples and returns
    /// an iterator of corresponding scores.
    ///
    /// For a less ergonomic version, see `Midas::iterate`.
    ///
    /// # Panics
    ///
    /// Subsequent iterator will panic if ever passed a thruple where
    /// the third element (the time) decreases from its predecessor.
    fn midas(self, params: MidasParams) -> Box<dyn 'a + Iterator<Item = Float>> {
        Box::new(Midas::iterate(self, params))
    }

    fn thing() {
        let iter = vec![(1, 1, 1), (1, 2, 1), (1, 1, 3), (1, 2, 4)]
            .into_iter()
            .midas_r(Default::default());

        for value in iter {
            println!("{:.6}", value);
        }
    }

    /// Takes an iterator of `(source, dest, time)` thruples and returns
    /// an iterator of corresponding scores.
    ///
    /// For a less ergonomic version, see `MidasR::iterate`.
    ///
    /// ```rust
    /// # fn main() {
    /// use midas_rs::MidasIterator;
    ///
    /// let iter = vec![(1, 1, 1), (1, 2, 1), (1, 1, 3), (1, 2, 4)]
    ///     .into_iter()
    ///     .midas_r(Default::default());
    ///
    /// for value in iter {
    ///     println!("{:.6}", value);
    /// }
    /// # }
    /// ```
    ///
    /// # Panics
    ///
    /// Subsequent iterator will panic if ever passed a thruple where
    /// the third element (the time) decreases from its predecessor.
    fn midas_r(self, params: MidasRParams) -> Box<dyn 'a + Iterator<Item = Float>> {
        Box::new(MidasR::iterate(self, params))
    }
}

impl<'a, T> MidasIterator<'a> for T where T: 'a + Iterator<Item = (Int, Int, Int)> + Sized {}
