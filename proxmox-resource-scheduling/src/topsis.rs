use anyhow::{bail, Error};

fn differences<const N: usize>(xs: &[f64; N], ys: &[f64; N]) -> [f64; N] {
    let mut differences = [0.0; N];
    for n in 0..N {
        differences[n] = xs[n] - ys[n];
    }
    differences
}

/// Calculate the L^2-norm of the given values.
fn l2_norm(values: &[f64]) -> f64 {
    values.into_iter().map(|v| v * v).sum::<f64>().sqrt()
}

/// A criterion that can be used when scoring with the TOPSIS algorithm.
pub struct Criterion {
    /// Name of the criterion.
    name: String,
    /// The relative weight of the criterion. Is non-negative.
    weight: f64,
    /// Whether it's good to maximize or minimize the criterion.
    maximize: bool,
}

impl Criterion {
    /// Construct a new `Criterion`. Use a negative weight if the value for the criterion
    /// should be minimized rather than maximized.
    ///
    /// Assumes that `weight` is finite.
    pub fn new(name: String, weight: f64) -> Self {
        let (maximize, weight) = if weight >= 0.0 {
            (true, weight)
        } else {
            (false, -weight)
        };

        Criterion {
            name,
            weight,
            maximize,
        }
    }
}

/// A normalized array of `Criterion`.
pub struct Criteria<const N_CRITERIA: usize>([Criterion; N_CRITERIA]);

impl<const N: usize> Criteria<N> {
    /// Create a new instance of normalized TOPSIS criteria.
    ///
    /// Assumes that the sum of weights can be calculated to a finite, non-zero value.
    pub fn new(mut criteria: [Criterion; N]) -> Result<Self, Error> {
        let divisor: f64 = criteria.iter().map(|c| c.weight).sum();

        if divisor == 0.0 {
            bail!("no criterion has a non-zero weight");
        }

        for criterion in criteria.iter_mut() {
            criterion.weight /= divisor;
            if criterion.weight > 1.0 || criterion.weight < 0.0 || !criterion.weight.is_finite() {
                bail!(
                    "criterion '{}' got invalid weight {}",
                    criterion.name,
                    criterion.weight
                );
            }
        }

        Ok(Criteria(criteria))
    }

    /// Weigh each value according to the weight of its corresponding criterion.
    pub fn weigh(&self, mut values: [f64; N]) -> [f64; N] {
        for (n, value) in values.iter_mut().enumerate() {
            *value *= self.0[n].weight;
        }
        values
    }
}

/// A normalized matrix used for scoring with the TOPSIS algorithm.
pub struct Matrix<const N_CRITERIA: usize>(Vec<[f64; N_CRITERIA]>);

impl<const N: usize> Matrix<N> {
    /// Values of the matrix for the fixed critierion with index `index`.
    fn fixed_criterion(&self, index: usize) -> Vec<f64> {
        self.0
            .iter()
            .map(|alternative| alternative[index])
            .collect::<Vec<_>>()
    }

    /// Mutable values of the matrix for the fixed critierion with index `index`.
    fn fixed_criterion_mut(&mut self, index: usize) -> Vec<&mut f64> {
        self.0
            .iter_mut()
            .map(|alternative| &mut alternative[index])
            .collect::<Vec<&mut _>>()
    }

    /// Create a normalized `Matrix` based on the given values.
    ///
    /// Assumes that the sum of squares for each fixed criterion in `matrix` can be calculated to a
    /// finite value.
    pub fn new(matrix: Vec<[f64; N]>) -> Result<Self, Error> {
        let mut matrix = Matrix(matrix);
        for n in 0..N {
            let divisor = l2_norm(&matrix.fixed_criterion(n));

            // If every alternative has zero value for the given criterion, keep it like that.
            if divisor != 0.0 {
                for value in matrix.fixed_criterion_mut(n).into_iter() {
                    *value /= divisor;
                    if !value.is_finite() {
                        bail!("criterion {} got invalid value {}", n, value);
                    }
                }
            }
        }

        Ok(matrix)
    }
}

/// Idealized alternatives from a `Matrix`. That is, the alternative consisting of the best
/// (respectively worst) value among the alternatives in the matrix for each single criterion.
struct IdealAlternatives<const N_CRITERIA: usize> {
    best: [f64; N_CRITERIA],
    worst: [f64; N_CRITERIA],
}

impl<const N: usize> IdealAlternatives<N> {
    /// Compute the idealized alternatives from the given `matrix`. The `criteria` are required to know
    /// if a critierion should be maximized or minimized.
    fn compute(matrix: &Matrix<N>, criteria: &Criteria<N>) -> Self {
        let criteria = &criteria.0;

        let mut best = [0.0; N];
        let mut worst = [0.0; N];

        for n in 0..N {
            let fixed_criterion = matrix.fixed_criterion(n);
            let min = fixed_criterion
                .iter()
                .min_by(|a, b| a.total_cmp(b))
                .unwrap();
            let max = fixed_criterion
                .iter()
                .max_by(|a, b| a.total_cmp(b))
                .unwrap();

            (best[n], worst[n]) = match criteria[n].maximize {
                true => (*max, *min),
                false => (*min, *max),
            }
        }

        Self { best, worst }
    }
}

/// Scores the alternatives in `matrix` according to their similarity to the ideal worst
/// alternative. Scores range from 0.0 to 1.0 and a low score indicates closness to the ideal worst
/// and/or distance to the ideal best alternatives.
pub fn score_alternatives<const N: usize>(
    matrix: &Matrix<N>,
    criteria: &Criteria<N>,
) -> Result<Vec<f64>, Error> {
    let ideal = IdealAlternatives::compute(matrix, criteria);

    let mut scores = vec![];

    for alternative in matrix.0.iter() {
        let distance_to_best = l2_norm(&criteria.weigh(differences(alternative, &ideal.best)));
        let distance_to_worst = l2_norm(&criteria.weigh(differences(alternative, &ideal.worst)));

        let divisor = distance_to_worst + distance_to_best;
        if divisor == 0.0 {
            // Can happen if all alternatives are the same.
            scores.push(0.0);
        } else {
            scores.push(distance_to_worst / divisor);
        }
    }

    if let Some(score) = scores.iter().find(|score| !score.is_finite()) {
        bail!("invalid score {}", score);
    }

    Ok(scores)
}

/// Similar to `score_alternatives`, but returns the list of indices decreasing by score.
pub fn rank_alternatives<const N: usize>(
    matrix: &Matrix<N>,
    criteria: &Criteria<N>,
) -> Result<Vec<usize>, Error> {
    let scores = score_alternatives(matrix, criteria)?;
    let mut indices: Vec<usize> = (0..scores.len()).collect();
    indices.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]));
    Ok(indices)
}

#[macro_export]
macro_rules! criteria_struct {
    (@count: $field:ident $($more:ident)*) => {
        1 + $crate::criteria_struct!(@count: $($more)*)
    };
    (@count: ) => { 0 };
    (
        $(#[$attr:meta])*
        struct $name:ident {
            $(
                #[criterion($crit_name:literal, $crit_weight:literal)]
                $(#[$field_attr:meta])*
                $field:ident : $type:ty,
            )*
        }
        const $count_name:ident;
        static $criteria_name:ident;
    ) => {
        const $count_name: usize = $crate::criteria_struct!(@count: $($field)*);

        $(#[$attr])*
        struct $name {
            $(
                $(#[$field_attr])*
                $field: $type,
            )*
        }

        ::lazy_static::lazy_static! {
            static ref $criteria_name: $crate::topsis::Criteria<$count_name> =
                $crate::topsis::Criteria::new([
                    $(
                        $crate::topsis::Criterion::new($crit_name.to_string(), $crit_weight),
                    )*
                ])
                .unwrap();
        }

        impl From<$name> for [f64; $count_name] {
            fn from(alternative: $name) -> Self {
                [ $( alternative.$field, )* ]
            }
        }
    };
}
