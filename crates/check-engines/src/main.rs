fn main() {
    let package = finder::get_package().unwrap();
    let package_lock = finder::get_most_recently_modified_lock().unwrap();
    let parsed_package = parser::parse_package(&package).unwrap();

    println!("Package content: {:?}", parsed_package);

    let parsed_lock_package = parser::parse_lock(&package_lock).unwrap();

    println!("Lock content: {:?}", parsed_lock_package);
}
