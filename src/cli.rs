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

use colored::Colorize;

pub trait LogStyle: ToString {
    fn announce(&self) -> colored::ColoredString { self.to_string().bold().bright_blue() }

    fn announcer(&self) -> colored::ColoredString { self.to_string().italic().bright_blue() }

    fn action(&self) -> colored::ColoredString { self.to_string().bold().yellow() }

    fn progress(&self) -> colored::ColoredString { self.to_string().bold().green() }

    fn ended(&self) -> colored::ColoredString { self.to_string().bold().bright_green() }

    fn actor(&self) -> colored::ColoredString { self.to_string().italic().bright_green() }

    fn amount(&self) -> colored::ColoredString { self.to_string().bold().bright_yellow() }

    fn addr(&self) -> colored::ColoredString { self.to_string().bold().bright_yellow() }

    fn err(&self) -> colored::ColoredString { self.to_string().bold().bright_red() }

    fn err_details(&self) -> colored::ColoredString { self.to_string().bold().red() }
}

impl<T> LogStyle for T where T: ToString {}
