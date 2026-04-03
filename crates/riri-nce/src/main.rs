mod finder;

use riri_npm::{parse_lock, parse_package};

fn main() {
    let package =
        finder::get_package().expect("Unable to get package.json file in the current directory");
    let package_lock = finder::get_most_recently_modified_lock()
        .expect("Unable to get the most recently modified lock file in the current directory");
    let (parsed_package, _, _) =
        parse_package(&package).expect("Unable to parse package.json file");

    println!("Package content: {parsed_package:?}");

    let parsed_lock_package = parse_lock(&package_lock).expect("Unable to parse lock file");

    println!("Lock content: {parsed_lock_package:?}");
}
