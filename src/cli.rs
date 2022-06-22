// LNP/BP Core Library implementing LNPBP specifications & standards
// Written in 2020 by
//     Dr. Maxim Orlovsky <orlovsky@pandoracore.com>
//
// To the extent possible under law, the author(s) have dedicated all
// copyright and related and neighboring rights to this software to
// the public domain worldwide. This software is distributed without
// any warranty.
//
// You should have received a copy of the MIT License
// along with this software.
// If not, see <https://opensource.org/licenses/MIT>.

use std::io::{Read, Write};
use std::path::Path;
use std::{fs, io};

use colored::Colorize;

pub trait LogStyle: ToString {
    fn c_id(&self) -> colored::ColoredString { self.to_string().italic().bright_blue() }

    fn c_start(&self) -> colored::ColoredString { self.to_string().bright_blue() }

    fn c_progr(&self) -> colored::ColoredString { self.to_string().green() }

    fn c_succ(&self) -> colored::ColoredString { self.to_string().bold().bright_green() }

    fn c_val(&self) -> colored::ColoredString { self.to_string().bold().bright_yellow() }

    fn c_addr(&self) -> colored::ColoredString { self.to_string().yellow() }

    fn c_err(&self) -> colored::ColoredString { self.to_string().bold().bright_red() }

    fn c_warn(&self) -> colored::ColoredString { self.to_string().bold().bright_yellow() }

    fn c_info(&self) -> colored::ColoredString { self.to_string().bold() }
}

impl<T> LogStyle for T where T: ToString {}

pub fn open_file_or_stdin(filename: Option<impl AsRef<Path>>) -> Result<Box<dyn Read>, io::Error> {
    Ok(match filename {
        Some(filename) => {
            let file = fs::File::open(filename)?;
            Box::new(file)
        }
        None => Box::new(io::stdin()),
    })
}

pub fn open_file_or_stdout(
    filename: Option<impl AsRef<Path>>,
) -> Result<Box<dyn Write>, io::Error> {
    Ok(match filename {
        Some(filename) => {
            let file = fs::File::create(filename)?;
            Box::new(file)
        }
        None => Box::new(io::stdout()),
    })
}

pub fn read_file_or_stdin(filename: Option<impl AsRef<Path>>) -> Result<Vec<u8>, io::Error> {
    let mut reader = open_file_or_stdin(filename)?;
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf)?;
    Ok(buf)
}

pub fn write_file_or_stdout(
    data: impl AsRef<[u8]>,
    filename: Option<impl AsRef<Path>>,
) -> Result<(), io::Error> {
    let mut writer = open_file_or_stdout(filename)?;
    writer.write_all(data.as_ref())
}
