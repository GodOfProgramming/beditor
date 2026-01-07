pub mod ecs;
pub mod extensions;
pub mod serde;
pub mod types;

use bevy::ecs::system::SystemParam;

#[derive(SystemParam)]
pub struct NoParams;

#[macro_export]
macro_rules! match_else {
	($value:ident; else $err:ident => $blk:block) => {
		match $value {
			Ok(_tmp_) => _tmp_,
			Err($err) => $blk,
		}
	};
	($value:expr; else $err:ident => $blk:block) => {
		match $value {
			Ok(_tmp_) => _tmp_,
			Err($err) => $blk,
		}
	};
	($value:block; else $err:ident => $blk:block) => {
		match $value {
			Ok(_tmp_) => _tmp_,
			Err($err) => $blk,
		}
	};
}

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
