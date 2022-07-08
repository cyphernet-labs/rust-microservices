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

use std::fmt::{self, Display, Formatter};

use amplify::Wrapper;

#[derive(Wrapper, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, From, Default)]
#[derive(NetworkEncode, NetworkDecode)]
pub struct OptionDetails(pub Option<String>);

impl Display for OptionDetails {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.as_inner() {
            None => Ok(()),
            Some(msg) => write!(f, "; \"{}\"", msg),
        }
    }
}

impl OptionDetails {
    pub fn with(s: impl ToString) -> Self { Self(Some(s.to_string())) }
    pub fn new() -> Self { Self(None) }
    pub fn none() -> Self { Self(None) }
}

impl From<String> for OptionDetails {
    fn from(s: String) -> Self { OptionDetails(Some(s)) }
}

impl From<&str> for OptionDetails {
    fn from(s: &str) -> Self { OptionDetails(Some(s.to_string())) }
}
