




#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Time {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateTime {
    pub date: Date,
    pub time: Time,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Weekday(pub u8);

impl Weekday {
    pub const MON: Self = Self(0);
    pub const TUE: Self = Self(1);
    pub const WED: Self = Self(2);
    pub const THU: Self = Self(3);
    pub const FRI: Self = Self(4);
    pub const SAT: Self = Self(5);
    pub const SUN: Self = Self(6);

    pub fn short_name(self) -> &'static str {
        ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"][self.0 as usize % 7]
    }
}




pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}


pub fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}



pub fn first_weekday_of_month(year: i32, month: u8) -> Weekday {
    let t: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if month < 3 { year - 1 } else { year };
    let m = month as i32;
    let d = 1i32;
    let raw = (y + y / 4 - y / 100 + y / 400 + t[(m - 1) as usize] + d) % 7;

    Weekday(((raw + 6) % 7) as u8)
}


pub fn weekday_of(date: Date) -> Weekday {
    let t: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if date.month < 3 {
        date.year - 1
    } else {
        date.year
    };
    let m = date.month as i32;
    let d = date.day as i32;
    let raw = (y + y / 4 - y / 100 + y / 400 + t[(m - 1) as usize] + d) % 7;
    Weekday(((raw + 6) % 7) as u8)
}


pub fn today() -> Date {

    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    date_from_unix_days(secs / 86400)
}


pub fn now_time() -> Time {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    Time {
        hour: h as u8,
        minute: m as u8,
        second: s as u8,
    }
}


fn date_from_unix_days(days: i64) -> Date {

    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    Date {
        year: y as i32,
        month: m as u8,
        day: d as u8,
    }
}




pub fn validate_date(year: i32, month: u8, day: u8) -> Result<Date, String> {
    if month < 1 || month > 12 {
        return Err(format!("Invalid month: {month}"));
    }
    let max_day = days_in_month(year, month);
    if day < 1 || day > max_day {
        return Err(format!(
            "Invalid day {day} for {}/{year} (max {max_day})",
            month
        ));
    }
    Ok(Date { year, month, day })
}


pub fn validate_time(hour: u8, minute: u8, second: u8) -> Result<Time, String> {
    if hour > 23 {
        return Err(format!("Invalid hour: {hour}"));
    }
    if minute > 59 {
        return Err(format!("Invalid minute: {minute}"));
    }
    if second > 59 {
        return Err(format!("Invalid second: {second}"));
    }
    Ok(Time {
        hour,
        minute,
        second,
    })
}



impl Date {
    pub fn add_months(self, delta: i32) -> Self {
        let total = self.month as i32 - 1 + delta;
        let year = self.year + total.div_euclid(12);
        let month = (total.rem_euclid(12) + 1) as u8;
        let day = self.day.min(days_in_month(year, month));
        Date { year, month, day }
    }

    pub fn add_days(self, delta: i32) -> Self {

        let mut d = self;
        let step = if delta >= 0 { 1i32 } else { -1 };
        let mut remaining = delta.abs();
        while remaining > 0 {
            if step > 0 {
                let max = days_in_month(d.year, d.month);
                if d.day < max {
                    d.day += 1;
                } else {
                    d = Date {
                        year: d.year,
                        month: d.month,
                        day: max,
                    }
                    .add_months(1);
                    d.day = 1;
                }
            } else {
                if d.day > 1 {
                    d.day -= 1;
                } else {
                    d = d.add_months(-1);
                    d.day = days_in_month(d.year, d.month);
                }
            }
            remaining -= 1;
        }
        d
    }


    pub fn from_parts(year: i32, month: u8, day: u8) -> Result<Self, String> {
        validate_date(year, month, day)
    }


    pub fn to_iso(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl Time {
    pub fn from_parts(hour: u8, minute: u8, second: u8) -> Result<Self, String> {
        validate_time(hour, minute, second)
    }


    pub fn to_iso(self) -> String {
        format!("{:02}:{:02}:{:02}", self.hour, self.minute, self.second)
    }


    pub fn to_hhmm(self) -> String {
        format!("{:02}:{:02}", self.hour, self.minute)
    }
}

impl DateTime {
    pub fn to_iso(self) -> String {
        format!("{}T{}", self.date.to_iso(), self.time.to_iso())
    }
}





pub struct MonthGrid {
    pub year: i32,
    pub month: u8,

    pub cells: [[Option<u8>; 7]; 6],
}

impl MonthGrid {
    pub fn new(year: i32, month: u8) -> Self {
        let first_wd = first_weekday_of_month(year, month).0 as usize;
        let days = days_in_month(year, month) as usize;
        let mut cells = [[None; 7]; 6];
        for day in 1..=days {
            let pos = first_wd + day - 1;
            cells[pos / 7][pos % 7] = Some(day as u8);
        }
        MonthGrid { year, month, cells }
    }

    pub fn month_name(&self) -> &'static str {
        [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ][(self.month as usize).saturating_sub(1) % 12]
    }
}




pub mod fmt {
    pub const DATE_DMY: &str = "DD/MM/YYYY";
    pub const DATE_MDY: &str = "MM/DD/YYYY";
    pub const DATE_YMD: &str = "YYYY-MM-DD";
    pub const TIME_HM: &str = "HH:mm";
    pub const TIME_HMS: &str = "HH:mm:ss";
    pub const DATETIME_DMY_HM: &str = "DD/MM/YYYY HH:mm";
    pub const DATETIME_YMD_HM: &str = "YYYY-MM-DD HH:mm";
    pub const DATETIME_DMY_HMS: &str = "DD/MM/YYYY HH:mm:ss";
}
