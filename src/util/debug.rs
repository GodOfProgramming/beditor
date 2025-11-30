#[macro_export]
macro_rules! here {
  () => {{
    use std::io::Write;
    println!("{}({})", file!(), line!());
    std::io::stdout().flush().ok();
  }};

  ($($arg:tt)*) => {{
    use std::io::Write;
    print!("{}({}): ", file!(), line!());
    std::io::stdout().flush().ok();
    println!($($arg)*);
  }};
}
