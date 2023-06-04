
#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc, NaiveDateTime};

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Cli::command().debug_assert()
    }

    #[test]
    fn test_date_parsing() {
        let nt = NaiveDateTime::from_timestamp_opt(1685871491, 0);
        let dt: DateTime<Utc> = DateTime::from_utc(nt.unwrap(), Utc);
        assert_eq!(
            parse_time("2023-06-04T09:38:11.000000000+00:00".to_string()).unwrap(),
            dt
        );
        assert_eq!(
            parse_time("2023-06-04T09:38:11.000000000+00:00\n".to_string()).unwrap(),
            dt
        );
        assert_eq!(
            parse_time("2030604T09:xy:11.000000000+00:00".to_string()).is_err(),
            true
        )
    }

}

