/// The datetime coordinates
use chrono::{Date, DateTime, Datelike, Duration, NaiveTime, TimeZone, Timelike};
use std::ops::Range;

use super::{AsRangedCoord, DescreteRanged, Ranged};

/// The trait that describe some time value
pub trait TimeValue: Eq {
    type Tz: TimeZone;
    /// Returns the date that is no later than the time
    fn date_floor(&self) -> Date<Self::Tz>;
    /// Returns the date that is no earlier than the time
    fn date_ceil(&self) -> Date<Self::Tz>;
    /// Returns the maximum value that is eariler than the given date
    fn earliest_after_date(date: Date<Self::Tz>) -> Self;
    /// Returns the duration between two time vlaue
    fn subtract(&self, other: &Self) -> Duration;
    /// Get the timezone information for current value
    fn timezone(&self) -> Self::Tz;

    /// Map the coord
    fn map_coord(value: &Self, begin: &Self, end: &Self, limit: (i32, i32)) -> i32 {
        let total_span = end.subtract(begin);
        let value_span = value.subtract(begin);

        // First, lets try the nanoseconds precision
        if let Some(total_ns) = total_span.num_nanoseconds() {
            if let Some(value_ns) = value_span.num_nanoseconds() {
                return (f64::from(limit.1 - limit.0) * value_ns as f64 / total_ns as f64) as i32
                    + limit.0;
            }
        }

        // If it overflows, it means we have a timespan nearly 300 years, we are safe to ignore the
        // porition less than 1 day.
        let total_days = total_span.num_days() as f64;
        let value_days = value_span.num_days() as f64;

        (f64::from(limit.1 - limit.0) * value_days / total_days) as i32 + limit.0
    }
}

impl<Z: TimeZone> TimeValue for Date<Z> {
    type Tz = Z;
    fn date_floor(&self) -> Date<Z> {
        self.clone()
    }
    fn date_ceil(&self) -> Date<Z> {
        self.clone()
    }
    fn earliest_after_date(date: Date<Z>) -> Self {
        date
    }
    fn subtract(&self, other: &Date<Z>) -> Duration {
        self.clone() - other.clone()
    }
    fn timezone(&self) -> Self::Tz {
        self.timezone()
    }
}

impl<Z: TimeZone> TimeValue for DateTime<Z> {
    type Tz = Z;
    fn date_floor(&self) -> Date<Z> {
        self.date()
    }
    fn date_ceil(&self) -> Date<Z> {
        if self.time().num_seconds_from_midnight() > 0 {
            self.date() + Duration::days(1)
        } else {
            self.date()
        }
    }
    fn earliest_after_date(date: Date<Z>) -> DateTime<Z> {
        date.and_hms(0, 0, 0)
    }

    fn subtract(&self, other: &DateTime<Z>) -> Duration {
        self.clone() - other.clone()
    }
    fn timezone(&self) -> Self::Tz {
        self.timezone()
    }
}

/// The ranged coordinate for date
pub struct RangedDate<Z: TimeZone>(Date<Z>, Date<Z>);

impl<Z: TimeZone> From<Range<Date<Z>>> for RangedDate<Z> {
    fn from(range: Range<Date<Z>>) -> Self {
        Self(range.start, range.end)
    }
}

impl<Z: TimeZone> Ranged for RangedDate<Z> {
    type ValueType = Date<Z>;

    fn range(&self) -> Range<Date<Z>> {
        self.0.clone()..self.1.clone()
    }

    fn map(&self, value: &Self::ValueType, limit: (i32, i32)) -> i32 {
        TimeValue::map_coord(value, &self.0, &self.1, limit)
    }

    fn key_points(&self, max_points: usize) -> Vec<Self::ValueType> {
        let mut ret = vec![];

        let total_days = (self.1.clone() - self.0.clone()).num_days();
        let total_weeks = (self.1.clone() - self.0.clone()).num_weeks();

        if total_days > 0 && total_days as usize <= max_points {
            for day_idx in 0..=total_days {
                ret.push(self.0.clone() + Duration::days(day_idx));
            }
            return ret;
        }

        if total_weeks > 0 && total_weeks as usize <= max_points {
            for day_idx in 0..=total_weeks {
                ret.push(self.0.clone() + Duration::weeks(day_idx));
            }
            return ret;
        }

        let week_per_point = ((total_weeks as f64) / (max_points as f64)).ceil() as usize;

        for idx in 0..=(total_weeks as usize / week_per_point) {
            ret.push(self.0.clone() + Duration::weeks((idx * week_per_point) as i64));
        }

        ret
    }
}

impl<Z: TimeZone> DescreteRanged for RangedDate<Z> {
    fn next_value(this: &Date<Z>) -> Date<Z> {
        this.clone() + Duration::days(1)
    }

    fn previous_value(this: &Date<Z>) -> Date<Z> {
        this.clone() - Duration::days(1)
    }
}

impl<Z: TimeZone> AsRangedCoord for Range<Date<Z>> {
    type CoordDescType = RangedDate<Z>;
    type Value = Date<Z>;
}

/// Indicatets the coord has a monthly resolution
pub struct Monthly<T: TimeValue>(Range<T>);

impl<T: TimeValue + Clone> AsRangedCoord for Monthly<T> {
    type CoordDescType = Monthly<T>;
    type Value = T;
}

impl<T: TimeValue + Clone> Ranged for Monthly<T> {
    type ValueType = T;

    fn range(&self) -> Range<T> {
        self.0.start.clone()..self.0.end.clone()
    }

    fn map(&self, value: &Self::ValueType, limit: (i32, i32)) -> i32 {
        T::map_coord(value, &self.0.start, &self.0.end, limit)
    }

    fn key_points(&self, max_points: usize) -> Vec<Self::ValueType> {
        let start_date = self.0.start.date_ceil();
        let end_date = self.0.end.date_floor();

        let mut start_year = start_date.year();
        let mut start_month = start_date.month();
        let start_day = start_date.day();

        let end_year = end_date.year();
        let end_month = end_date.month();

        if start_day != 1 {
            start_month += 1;
            if start_month == 13 {
                start_month = 1;
                start_year += 1;
            }
        }

        let total_month = (end_year - start_year) * 12 + end_month as i32 - start_month as i32;

        fn generate_key_points<T: TimeValue>(
            mut start_year: i32,
            mut start_month: i32,
            end_year: i32,
            end_month: i32,
            step: u32,
            tz: T::Tz,
        ) -> Vec<T> {
            let mut ret = vec![];
            while end_year > start_year || (end_year == start_year && end_month >= start_month) {
                ret.push(T::earliest_after_date(tz.ymd(
                    start_year,
                    start_month as u32,
                    1,
                )));
                start_month += step as i32;

                if start_month >= 13 {
                    start_year += start_month / 12;
                    start_month %= 12;
                }
            }

            ret
        }

        if total_month as usize <= max_points {
            // Monthly
            return generate_key_points(
                start_year,
                start_month as i32,
                end_year,
                end_month as i32,
                1,
                self.0.start.timezone(),
            );
        } else if total_month as usize <= max_points * 3 {
            // Quaterly
            return generate_key_points(
                start_year,
                start_month as i32,
                end_year,
                end_month as i32,
                3,
                self.0.start.timezone(),
            );
        } else if total_month as usize <= max_points * 6 {
            // Biyearly
            return generate_key_points(
                start_year,
                start_month as i32,
                end_year,
                end_month as i32,
                6,
                self.0.start.timezone(),
            );
        }

        // Otherwise we could generate the yearly keypoints
        generate_yearly_keypoints(
            max_points,
            start_year,
            start_month,
            end_year,
            end_month,
            self.0.start.timezone(),
        )
    }
}

impl<T: TimeValue + Clone> DescreteRanged for Monthly<T> {
    fn next_value(this: &T) -> T {
        let mut year = this.date_ceil().year();
        let mut month = this.date_ceil().month();
        month += 1;
        if month == 13 {
            month = 1;
            year += 1;
        }
        T::earliest_after_date(this.timezone().ymd(year, month, this.date_ceil().day()))
    }

    fn previous_value(this: &T) -> T {
        let mut year = this.clone().date_floor().year();
        let mut month = this.clone().date_floor().month();
        month -= 1;
        if month == 0 {
            month = 12;
            year -= 1;
        }
        T::earliest_after_date(this.timezone().ymd(year, month, this.date_floor().day()))
    }
}

/// Indicate the coord has a yearly resolution
pub struct Yearly<T: TimeValue>(Range<T>);

impl<T: TimeValue + Clone> AsRangedCoord for Yearly<T> {
    type CoordDescType = Yearly<T>;
    type Value = T;
}

fn generate_yearly_keypoints<T: TimeValue>(
    max_points: usize,
    mut start_year: i32,
    start_month: u32,
    mut end_year: i32,
    end_month: u32,
    tz: T::Tz,
) -> Vec<T> {
    if start_month > end_month {
        end_year -= 1;
    }

    let mut exp10 = 1;

    while (end_year - start_year + 1) as usize / (exp10 * 10) > max_points {
        exp10 *= 10;
    }

    let mut freq = exp10;

    for try_freq in &[1, 2, 5, 10] {
        freq = *try_freq * exp10;
        if (end_year - start_year + 1) as usize / (exp10 * *try_freq) <= max_points {
            break;
        }
    }

    let mut ret = vec![];

    while start_year <= end_year {
        ret.push(T::earliest_after_date(tz.ymd(start_year, start_month, 1)));
        start_year += freq as i32;
    }

    ret
}

impl<T: TimeValue + Clone> Ranged for Yearly<T> {
    type ValueType = T;

    fn range(&self) -> Range<T> {
        self.0.start.clone()..self.0.end.clone()
    }

    fn map(&self, value: &Self::ValueType, limit: (i32, i32)) -> i32 {
        T::map_coord(value, &self.0.start, &self.0.end, limit)
    }

    fn key_points(&self, max_points: usize) -> Vec<Self::ValueType> {
        let start_date = self.0.start.date_ceil();
        let end_date = self.0.end.date_floor();

        let mut start_year = start_date.year();
        let mut start_month = start_date.month();
        let start_day = start_date.day();

        let end_year = end_date.year();
        let end_month = end_date.month();

        if start_day != 1 {
            start_month += 1;
            if start_month == 13 {
                start_month = 1;
                start_year += 1;
            }
        }

        generate_yearly_keypoints(
            max_points,
            start_year,
            start_month,
            end_year,
            end_month,
            self.0.start.timezone(),
        )
    }
}

impl<T: TimeValue + Clone> DescreteRanged for Yearly<T> {
    fn next_value(this: &T) -> T {
        T::earliest_after_date(this.timezone().ymd(this.date_floor().year() + 1, 1, 1))
    }

    fn previous_value(this: &T) -> T {
        T::earliest_after_date(this.timezone().ymd(this.date_ceil().year() - 1, 1, 1))
    }
}

/// The trait that converts a normal date coord into a yearly one
pub trait IntoMonthly<T: TimeValue> {
    fn monthly(self) -> Monthly<T>;
}

/// The trait that converts a normal date coord into a yearly one
pub trait IntoYearly<T: TimeValue> {
    fn yearly(self) -> Yearly<T>;
}

impl<T: TimeValue> IntoMonthly<T> for Range<T> {
    fn monthly(self) -> Monthly<T> {
        Monthly(self)
    }
}

impl<T: TimeValue> IntoYearly<T> for Range<T> {
    fn yearly(self) -> Yearly<T> {
        Yearly(self)
    }
}

/// The ranged coordinate for the date and time
pub struct RangedDateTime<Z: TimeZone>(DateTime<Z>, DateTime<Z>);

impl<Z: TimeZone> AsRangedCoord for Range<DateTime<Z>> {
    type CoordDescType = RangedDateTime<Z>;
    type Value = Date<Z>;
}

impl<Z: TimeZone> From<Range<DateTime<Z>>> for RangedDateTime<Z> {
    fn from(range: Range<DateTime<Z>>) -> Self {
        Self(range.start, range.end)
    }
}

impl<Z: TimeZone> Ranged for RangedDateTime<Z> {
    type ValueType = DateTime<Z>;

    fn range(&self) -> Range<DateTime<Z>> {
        self.0.clone()..self.1.clone()
    }

    fn map(&self, value: &Self::ValueType, limit: (i32, i32)) -> i32 {
        TimeValue::map_coord(value, &self.0, &self.1, limit)
    }

    fn key_points(&self, max_points: usize) -> Vec<Self::ValueType> {
        let total_span = self.1.clone() - self.0.clone();

        if let Some(total_ns) = total_span.num_nanoseconds() {
            if let Some(actual_ns_per_point) =
                compute_period_per_point(total_ns as u64, max_points, true)
            {
                let start_time_ns = u64::from(self.0.time().num_seconds_from_midnight())
                    * 1_000_000_000
                    + u64::from(self.0.time().nanosecond());

                let mut start_time = self
                    .0
                    .date_floor()
                    .and_time(
                        NaiveTime::from_hms(0, 0, 0)
                            + Duration::nanoseconds(if start_time_ns % actual_ns_per_point > 0 {
                                start_time_ns
                                    + (actual_ns_per_point - start_time_ns % actual_ns_per_point)
                            } else {
                                start_time_ns
                            } as i64),
                    )
                    .unwrap();

                let mut ret = vec![];

                while start_time < self.1 {
                    ret.push(start_time.clone());
                    start_time = start_time + Duration::nanoseconds(actual_ns_per_point as i64);
                }

                return ret;
            }
        }

        // Otherwise, it actually behaves like a date
        let date_range = RangedDate(self.0.date_ceil(), self.1.date_floor());

        date_range
            .key_points(max_points)
            .into_iter()
            .map(|x| x.and_hms(0, 0, 0))
            .collect()
    }
}

/// The coordinate that for duration of time
pub struct RangedDuration(Duration, Duration);

impl AsRangedCoord for Range<Duration> {
    type CoordDescType = RangedDuration;
    type Value = Duration;
}

impl From<Range<Duration>> for RangedDuration {
    fn from(range: Range<Duration>) -> Self {
        Self(range.start, range.end)
    }
}

impl Ranged for RangedDuration {
    type ValueType = Duration;

    fn range(&self) -> Range<Duration> {
        self.0..self.1
    }

    fn map(&self, value: &Self::ValueType, limit: (i32, i32)) -> i32 {
        let total_span = self.1 - self.0;
        let value_span = *value - self.0;

        if let Some(total_ns) = total_span.num_nanoseconds() {
            if let Some(value_ns) = value_span.num_nanoseconds() {
                return limit.0
                    + (f64::from(limit.1 - limit.0) * value_ns as f64 / total_ns as f64 + 1e-10)
                        as i32;
            }
            return limit.1;
        }

        let total_days = total_span.num_days();
        let value_days = value_span.num_days();

        limit.0
            + (f64::from(limit.1 - limit.0) * value_days as f64 / total_days as f64 + 1e-10) as i32
    }

    fn key_points(&self, max_points: usize) -> Vec<Self::ValueType> {
        let total_span = self.1 - self.0;

        if let Some(total_ns) = total_span.num_nanoseconds() {
            if let Some(period) = compute_period_per_point(total_ns as u64, max_points, false) {
                let mut start_ns = self.0.num_nanoseconds().unwrap();

                if start_ns as u64 % period > 0 {
                    if start_ns > 0 {
                        start_ns += period as i64 - (start_ns % period as i64);
                    } else {
                        start_ns -= start_ns % period as i64;
                    }
                }

                let mut current = Duration::nanoseconds(start_ns);
                let mut ret = vec![];

                while current < self.1 {
                    ret.push(current);
                    current = current + Duration::nanoseconds(period as i64);
                }

                return ret;
            }
        }

        let begin_days = self.0.num_days();
        let end_days = self.1.num_days();

        let mut days_per_tick = 1;
        let mut idx = 0;
        const MULTIPLER: &[i32] = &[1, 2, 5];

        while (end_days - begin_days) / i64::from(days_per_tick * MULTIPLER[idx])
            > max_points as i64
        {
            idx += 1;
            if idx == MULTIPLER.len() {
                idx = 0;
                days_per_tick *= 10;
            }
        }

        days_per_tick *= MULTIPLER[idx];

        let mut ret = vec![];

        let mut current = Duration::days(
            self.0.num_days()
                + if Duration::days(self.0.num_days()) != self.0 {
                    1
                } else {
                    0
                },
        );

        while current < self.1 {
            ret.push(current);
            current = current + Duration::days(i64::from(days_per_tick));
        }

        ret
    }
}

#[allow(clippy::inconsistent_digit_grouping)]
fn compute_period_per_point(total_ns: u64, max_points: usize, sub_daily: bool) -> Option<u64> {
    let min_ns_per_point = total_ns as f64 / max_points as f64;
    let actual_ns_per_point: u64 = (10u64).pow((min_ns_per_point as f64).log10().floor() as u32);

    fn deterime_actual_ns_per_point(
        total_ns: u64,
        mut actual_ns_per_point: u64,
        units: &[u64],
        base: u64,
        max_points: usize,
    ) -> u64 {
        let mut unit_per_point_idx = 0;
        while total_ns / actual_ns_per_point > max_points as u64 * units[unit_per_point_idx] {
            unit_per_point_idx += 1;
            if unit_per_point_idx == units.len() {
                unit_per_point_idx = 0;
                actual_ns_per_point *= base;
            }
        }
        units[unit_per_point_idx] * actual_ns_per_point
    }

    if actual_ns_per_point < 1_000_000_000 {
        Some(deterime_actual_ns_per_point(
            total_ns as u64,
            actual_ns_per_point,
            &[1, 2, 5],
            10,
            max_points,
        ))
    } else if actual_ns_per_point < 3600_000_000_000 {
        Some(deterime_actual_ns_per_point(
            total_ns as u64,
            1_000_000_000,
            &[1, 2, 5, 10, 15, 20, 30],
            60,
            max_points,
        ))
    } else if actual_ns_per_point < 3600_000_000_000 * 24 {
        Some(deterime_actual_ns_per_point(
            total_ns as u64,
            3600_000_000_000,
            &[1, 2, 4, 8, 12],
            24,
            max_points,
        ))
    } else if !sub_daily {
        if actual_ns_per_point < 3600_000_000_000 * 24 * 10 {
            Some(deterime_actual_ns_per_point(
                total_ns as u64,
                3600_000_000_000 * 24,
                &[1, 2, 5, 7],
                10,
                max_points,
            ))
        } else {
            Some(deterime_actual_ns_per_point(
                total_ns as u64,
                3600_000_000_000 * 24 * 10,
                &[1, 2, 5],
                10,
                max_points,
            ))
        }
    } else {
        None
    }
}
