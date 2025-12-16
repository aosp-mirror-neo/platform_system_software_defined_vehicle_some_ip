// Copyright (C) 2026 The Android Open Source Project
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::HashMap;
use std::collections::VecDeque;
use std::fmt;
use std::fmt::Display;
use std::sync::Mutex;
use tokio::time::Duration;
use tokio::time::Instant;

use crate::benchmark::benchmark_api::IBenchmark;
use crate::benchmark::benchmark_api::IBenchmarkArgument;

/// A benchmark implementation that collects and reports statistics on
/// measurements, such as average, min, max, median, and p90.
///
/// The benchmark maintains a window of measurements for each event key.
///
/// The `add_measurement` method is used to add a new measurement to the
/// benchmark. The `Display` implementation provides a formatted report of
/// the collected statistics and is used for the `dump()`.
///
/// The `event_window_map` contains a FIFO of events per key.
/// The stop time and the duration of events are stored.
/// (in the case of the SOME/IP Broker, the key is the unit key)
pub(crate) struct BenchmarkDumpsys<T: IBenchmarkArgument> {
    event_window_map: Mutex<HashMap<T, VecDeque<(Instant, Duration)>>>,
    max_delta_t: Duration,
}

impl<T: IBenchmarkArgument> IBenchmark<T> for BenchmarkDumpsys<T> {
    fn add_measurement(&self, key: &T, start: Instant, stop: Instant) {
        let Some(duration) = stop.checked_duration_since(start) else {
            return;
        };

        let mut event_window_map = self.event_window_map.lock().unwrap();
        let window = if let Some(existing_window) = event_window_map.get_mut(key) {
            self.clear_old_messages(existing_window);
            existing_window
        } else {
            event_window_map.entry(key.clone()).or_default()
        };

        self.clear_old_messages(window);

        window.push_back((stop, duration));
    }
}

fn average<'a>(window: impl Iterator<Item = &'a Duration>) -> Option<Duration> {
    let mut sum = Duration::new(0, 0);
    let mut counter = 0;
    for sample in window {
        sum = sum.checked_add(*sample)?;
        counter += 1;
    }
    sum.checked_div(counter)
}

fn sum<'a>(window: impl Iterator<Item = &'a Duration>) -> Option<Duration> {
    let mut sum = Duration::new(0, 0);
    for sample in window {
        sum = sum.checked_add(*sample)?;
    }
    Some(sum)
}

fn median<'a>(sorted_samples: &'a Vec<&'a Duration>) -> Option<Duration> {
    if sorted_samples.is_empty() {
        return None;
    }

    if sorted_samples.len().is_multiple_of(2) {
        let left = sorted_samples.iter().clone().nth(sorted_samples.len() / 2 - 1);
        let right = sorted_samples.iter().clone().nth(sorted_samples.len() / 2);
        left?.checked_add(**right?)?.checked_div(2)
    } else {
        sorted_samples.iter().clone().nth(sorted_samples.len() / 2).copied().copied()
    }
}

fn p90<'a>(sorted_samples: &'a Vec<&'a Duration>) -> Option<Duration> {
    if sorted_samples.is_empty() {
        return None;
    }

    if sorted_samples.len().is_multiple_of(10) {
        sorted_samples.iter().clone().nth((sorted_samples.len() * 9) / 10 - 1).copied().copied()
    } else {
        sorted_samples.iter().clone().nth((sorted_samples.len() * 9) / 10).copied().copied()
    }
}

fn print_duration(duration: Option<Duration>) -> String {
    match duration {
        Some(duration) => format!("{} us", duration.as_micros()),
        None => "(overflow)".to_owned(),
    }
}

impl<T: IBenchmarkArgument> Display for BenchmarkDumpsys<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut summary = Vec::new();
        let mut window_map = self.event_window_map.lock().unwrap();

        for (key, window) in window_map.iter_mut() {
            self.clear_old_messages(window);

            let samples_iterator = window.iter().map(|(_stop, duration)| duration);
            let mut sorted_samples = samples_iterator.clone().collect::<Vec<_>>();
            sorted_samples.sort_unstable();

            writeln!(f, "Statistics for {key}:")?;
            writeln!(f, "Messages Counter = {}", window.len())?;
            writeln!(f, "Sum = {}", print_duration(sum(samples_iterator.clone())))?;
            writeln!(f, "Average = {}", print_duration(average(samples_iterator.clone())))?;
            writeln!(f, "Min = {}", print_duration(samples_iterator.clone().min().copied()))?;
            writeln!(f, "Max = {}", print_duration(samples_iterator.clone().max().copied()))?;
            writeln!(f, "Median = {}", print_duration(median(&sorted_samples)))?;
            writeln!(f, "P90 = {}", print_duration(p90(&sorted_samples)))?;

            if let Some(sum) = sum(samples_iterator.clone()) {
                summary.push((sum, key));
            }
        }

        summary.sort_by(|(a, _), (b, _)| b.cmp(a));
        writeln!(f, "Summary (accumulated time in window):")?;
        for (total_duration, key) in summary.iter() {
            writeln!(f, "{key}: {} us", total_duration.as_micros())?;
        }

        Ok(())
    }
}

impl<T: IBenchmarkArgument> BenchmarkDumpsys<T> {
    pub(crate) fn new(max_delta_t: Duration) -> Self {
        Self { max_delta_t, event_window_map: Mutex::new(HashMap::new()) }
    }

    /// Pop elements from the window to ensure window's delta-t
    fn clear_old_messages(&self, window: &mut VecDeque<(Instant, Duration)>) {
        let curr_time = Instant::now();
        loop {
            let Some(front) = window.front().map(|(stop, _duration)| stop) else {
                break;
            };
            let Some(delta_t) = curr_time.checked_duration_since(*front) else {
                break;
            };
            if delta_t > self.max_delta_t {
                window.pop_front();
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_element() {
        let data = [Duration::from_secs(5)];
        assert_eq!(average(data.iter()), Some(Duration::from_secs(5)));
    }

    #[test]
    fn test_multiple_elements() {
        let data = [Duration::from_secs(2), Duration::from_secs(4), Duration::from_secs(6)];
        assert_eq!(average(data.iter()), Some(Duration::from_secs(4)));
    }

    #[test]
    fn test_mixed_units() {
        let data = [
            Duration::from_millis(1000), // 1 second
            Duration::from_secs(2),
            Duration::from_micros(3000000), // 3 seconds
        ];
        assert_eq!(average(data.iter()), Some(Duration::from_secs(2)));
    }

    #[test]
    fn test_large_durations_no_overflow() {
        let d1 = Duration::from_secs(u64::MAX / 3);
        let d2 = Duration::from_secs(u64::MAX / 3);
        let d3 = Duration::from_secs(u64::MAX / 3);
        let data = [d1, d2, d3];
        assert_eq!(average(data.iter()), Some(Duration::from_secs(u64::MAX / 3)));
    }

    #[test]
    fn test_addition_overflow() {
        let d1 = Duration::from_secs(u64::MAX / 2 + 1);
        let d2 = Duration::from_secs(u64::MAX / 2 + 1);
        let data = [d1, d2];
        assert_eq!(average(data.iter()), None);
    }

    #[test]
    fn test_zero_duration() {
        let data = [Duration::from_secs(1), Duration::from_secs(0), Duration::from_secs(2)];
        assert_eq!(average(data.iter()), Some(Duration::from_secs(1)));
    }

    #[test]
    fn test_empty_iterator() {
        let empty: Vec<Duration> = vec![];
        assert_eq!(sum(empty.iter()), Some(Duration::new(0, 0)));
    }

    #[test]
    fn test_multiple_elements_no_overflow() {
        let data =
            [Duration::new(1, 0), Duration::new(2, 500_000_000), Duration::new(0, 750_000_000)];
        assert_eq!(sum(data.iter()), Some(Duration::new(4, 250_000_000)));
    }

    #[test]
    fn test_multiple_elements_with_seconds_carry() {
        let data = [Duration::new(1, 700_000_000), Duration::new(0, 500_000_000)];
        assert_eq!(sum(data.iter()), Some(Duration::new(2, 200_000_000)));
    }

    #[test]
    fn test_potential_overflow() {
        let max_duration = Duration::MAX;
        let data = [max_duration, Duration::new(1, 0)];
        assert_eq!(sum(data.iter()), None);
    }

    #[test]
    fn test_sum_of_max_durations_overflows() {
        let max_duration = Duration::MAX;
        let data = [max_duration, max_duration];
        assert_eq!(sum(data.iter()), None);
    }

    // Tests for median function
    #[test]
    fn test_median_empty_vec() {
        let empty: Vec<&Duration> = vec![];
        assert_eq!(median(&empty), None);
    }

    #[test]
    fn test_median_single_element() {
        let d1 = Duration::new(5, 0);
        let data = vec![&d1];
        assert_eq!(median(&data), Some(Duration::new(5, 0)));
    }

    #[test]
    fn test_median_odd_number_of_elements() {
        let d1 = Duration::new(1, 0);
        let d2 = Duration::new(3, 0);
        let d3 = Duration::new(5, 0);
        let data = vec![&d1, &d2, &d3];
        assert_eq!(median(&data), Some(Duration::new(3, 0)));
    }

    #[test]
    fn test_median_even_number_of_elements_no_carry() {
        let d1 = Duration::new(1, 0);
        let d2 = Duration::new(4, 0);
        let data = vec![&d1, &d2];
        assert_eq!(median(&data), Some(Duration::new(2, 500_000_000)));
    }

    #[test]
    fn test_median_even_number_of_elements_with_carry() {
        let d1 = Duration::new(1, 600_000_000);
        let d2 = Duration::new(2, 800_000_000);
        let data = vec![&d1, &d2];
        assert_eq!(median(&data), Some(Duration::new(2, 200_000_000)));
    }

    #[test]
    fn test_median_even_number_of_elements_sum_overflow() {
        let max_duration = Duration::MAX;
        let almost_max = max_duration - Duration::new(0, 1);
        let data = vec![&almost_max, &max_duration];
        assert_eq!(median(&data), None);
    }

    #[test]
    fn test_median_even_number_of_elements_zero_sum() {
        let d1 = Duration::new(0, 0);
        let d2 = Duration::new(0, 0);
        let data = vec![&d1, &d2];
        assert_eq!(median(&data), Some(Duration::new(0, 0)));
    }

    #[test]
    fn test_median_larger_even_number_of_elements() {
        let d1 = Duration::new(1, 0);
        let d2 = Duration::new(2, 0);
        let d3 = Duration::new(3, 0);
        let d4 = Duration::new(4, 0);
        let data = vec![&d1, &d2, &d3, &d4];
        assert_eq!(median(&data), Some(Duration::new(2, 500_000_000)));
    }

    // Tests for p90 function
    #[test]
    fn test_p90_empty_vec() {
        let empty: Vec<&Duration> = vec![];
        assert_eq!(p90(&empty), None);
    }

    #[test]
    fn test_p90_single_element() {
        let d1 = Duration::new(5, 0);
        let data = vec![&d1];
        assert_eq!(p90(&data), Some(Duration::new(5, 0)));
    }

    #[test]
    fn test_p90_ten_elements() {
        let data_owned: Vec<Duration> = (1..=10).map(|i| Duration::new(i, 0)).collect();
        let data_ref: Vec<&Duration> = data_owned.iter().collect();
        assert_eq!(p90(&data_ref), Some(Duration::new(9, 0)));
    }

    #[test]
    fn test_p90_eleven_elements() {
        let data_owned: Vec<Duration> = (1..=11).map(|i| Duration::new(i, 0)).collect();
        let data_ref: Vec<&Duration> = data_owned.iter().collect();
        assert_eq!(p90(&data_ref), Some(Duration::new(10, 0)));
    }

    #[test]
    fn test_p90_less_than_ten_elements() {
        let d1 = Duration::new(1, 0);
        let d2 = Duration::new(2, 0);
        let d3 = Duration::new(3, 0);
        let data = vec![&d1, &d2, &d3];
        assert_eq!(p90(&data), Some(Duration::new(3, 0))); // (3 * 9) / 10 = 2
    }

    #[test]
    fn test_p90_larger_number_of_elements() {
        let data_owned: Vec<Duration> = (1..=100).map(|i| Duration::new(i, 0)).collect();
        let data_ref: Vec<&Duration> = data_owned.iter().collect();
        assert_eq!(p90(&data_ref), Some(Duration::new(90, 0)));
    }
}
