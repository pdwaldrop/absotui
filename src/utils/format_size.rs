/// Formats a byte count for display (e.g. `414MB`, `1.1GB`).
pub fn format_size(bytes: i64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let bytes_f = bytes as f64;

    if bytes_f >= GB {
        format!("{:.1}GB", bytes_f / GB)
    } else if bytes_f >= MB {
        format!("{:.0}MB", bytes_f / MB)
    } else if bytes_f >= KB {
        format!("{:.0}KB", bytes_f / KB)
    } else {
        format!("{bytes}B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_under_a_kb_shown_as_is() {
        assert_eq!(format_size(512), "512B");
    }

    #[test]
    fn megabytes_rounded_to_whole_number() {
        assert_eq!(format_size(434_346_364), "414MB");
    }

    #[test]
    fn gigabytes_shown_with_one_decimal() {
        assert_eq!(format_size(1_181_116_006), "1.1GB");
    }

    #[test]
    fn zero_is_zero_bytes() {
        assert_eq!(format_size(0), "0B");
    }
}
