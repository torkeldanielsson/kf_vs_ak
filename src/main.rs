use chrono::Datelike;
use chrono::NaiveDate;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;
use std::str::FromStr;

struct RecordOmxs30 {
    date: NaiveDate,
    value: f32,
}

fn parse_omxs30_line(line: &str) -> Result<RecordOmxs30, Box<dyn std::error::Error>> {
    let mut parts = line.split('\t');

    let date_str = parts.next().ok_or("Missing date")?;
    let value_str = parts.next().ok_or("Missing value")?;

    let value_clean_str = value_str.replace(' ', "").replace(',', ".");

    let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")?;
    let value = f32::from_str(&value_clean_str)?;

    Ok(RecordOmxs30 { date, value })
}

struct RecordSLR {
    date: NaiveDate,
    value: f32,
}

fn parse_slr_line(line: &str) -> Result<RecordSLR, Box<dyn std::error::Error>> {
    let parts: Vec<&str> = line.split(';').collect();

    if parts.len() < 3 {
        return Err("Not enough columns".into());
    }

    let date_str = parts[0];
    let value_str = parts[1]; // Third column for "Medelvärde hittills i år"

    let value_clean_str = value_str.replace(' ', "").replace(',', ".");

    let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")?;
    let value = f32::from_str(&value_clean_str)?;

    Ok(RecordSLR { date, value })
}

fn calculate_avkastningsskatt(slr: f32) -> f32 {
    let tax_base_rate = 0.30;
    let minimum_tax_percentage = 1.25;

    0.01 * (slr + 1.0).max(minimum_tax_percentage) * tax_base_rate
}

struct Record {
    avkastningsskatt: f32,
    omxs30: f32,
}

struct SeriesEntry {
    start_year: i32,
    aktiekonto: f32,
    kapitalförsäkring: f32,
}

fn print_series(len: i32, series: &[SeriesEntry]) {
    println!("\n{len} years:");
    let mut average_aktiekonto = 0.0;
    let mut average_kapitalförsäkring = 0.0;
    for e in series {
        average_aktiekonto += e.aktiekonto;
        average_kapitalförsäkring += e.kapitalförsäkring;
        println!(
            "{}:     {:.2}    {:.2}",
            e.start_year, e.aktiekonto, e.kapitalförsäkring
        );
    }
    println!(
        "{len} years averages:    {:.2}    {:.2}",
        average_aktiekonto / series.len() as f32,
        average_kapitalförsäkring / series.len() as f32
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut last_omxs30_by_year: HashMap<i32, f32> = HashMap::new();
    let mut last_slr_by_year: HashMap<i32, f32> = HashMap::new();

    {
        let path = Path::new("omxs30.txt");
        let file = File::open(path)?;
        let reader = io::BufReader::new(file);

        let mut records: Vec<RecordOmxs30> = reader
            .lines()
            .map_while(Result::ok)
            .filter_map(|line| parse_omxs30_line(&line).ok())
            .collect();

        records.sort_by(|a, b| a.date.cmp(&b.date));

        let mut last_records_by_year: HashMap<i32, &RecordOmxs30> = HashMap::new();
        for record in &records {
            last_records_by_year.insert(record.date.year(), record);
        }

        for (&year, record) in &last_records_by_year {
            last_omxs30_by_year.insert(year, record.value);
        }
    }

    {
        let path = Path::new("stadslåneränta.csv");
        let file = File::open(path)?;
        let reader = io::BufReader::new(file);

        let mut records: Vec<RecordSLR> = reader
            .lines()
            .skip(1) // Skip the header
            .map_while(Result::ok)
            .filter_map(|line| parse_slr_line(&line).ok())
            .collect();

        records.sort_by(|a, b| a.date.cmp(&b.date));

        let mut last_records_by_year: HashMap<i32, &RecordSLR> = HashMap::new();
        for record in &records {
            last_records_by_year.insert(record.date.year(), record);
        }

        for (&year, record) in &last_records_by_year {
            last_slr_by_year.insert(year, record.value);
        }
    }

    let mut combined_records = HashMap::new();
    let years: HashSet<_> = last_omxs30_by_year
        .keys()
        .chain(last_slr_by_year.keys())
        .collect();

    for year in years {
        let omxs30 = *last_omxs30_by_year.get(year).unwrap_or(&0.0);
        let slr = *last_slr_by_year.get(year).unwrap_or(&0.0);

        combined_records.insert(
            year,
            Record {
                avkastningsskatt: calculate_avkastningsskatt(slr),
                omxs30,
            },
        );
    }

    let mut series_5 = Vec::new();
    let mut series_10 = Vec::new();
    let mut series_15 = Vec::new();
    let mut series_20 = Vec::new();
    let mut series_25 = Vec::new();

    for start_year in 1993..=2022 {
        let mut kf_sum = 1.0;
        let mut ak_sum = 1.0;

        for year in (start_year + 1)..=2023 {
            let previous_val = combined_records[&(year - 1)].omxs30;
            let val = combined_records[&year].omxs30;
            let diff = val / previous_val;
            ak_sum *= diff;
            kf_sum *= diff;
            kf_sum *= 1.0 - combined_records[&year].avkastningsskatt;

            let ak_val = ak_sum - ((ak_sum - 1.0) * 0.206).max(0.0);

            let year_count = year - start_year;

            if year_count == 5 {
                series_5.push(SeriesEntry {
                    start_year,
                    aktiekonto: ak_val,
                    kapitalförsäkring: kf_sum,
                });
            }

            if year_count == 10 {
                series_10.push(SeriesEntry {
                    start_year,
                    aktiekonto: ak_val,
                    kapitalförsäkring: kf_sum,
                });
            }

            if year_count == 15 {
                series_15.push(SeriesEntry {
                    start_year,
                    aktiekonto: ak_val,
                    kapitalförsäkring: kf_sum,
                });
            }

            if year_count == 20 {
                series_20.push(SeriesEntry {
                    start_year,
                    aktiekonto: ak_val,
                    kapitalförsäkring: kf_sum,
                });
            }

            if year_count == 25 {
                series_25.push(SeriesEntry {
                    start_year,
                    aktiekonto: ak_val,
                    kapitalförsäkring: kf_sum,
                });
            }
        }
    }

    print_series(5, &series_5);
    print_series(10, &series_10);
    print_series(15, &series_15);
    print_series(20, &series_20);
    print_series(25, &series_25);

    Ok(())
}
