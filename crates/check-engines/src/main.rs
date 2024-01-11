fn main() {
    let package = finder::get_package().unwrap();
    let package_lock = finder::get_most_recently_modified_lock().unwrap();

    println!("Package: {:?}", package);
    println!("Lock: {:?}", package_lock);

    let parsed_package = parser::parse_package(&package).unwrap();

    println!("Package content: {:?}", parsed_package);

    match parser::parse_lock(&package_lock) {
        Ok(package_lock) => println!("Lock content: {:?}", package_lock),
        Err(err) => eprintln!("Error parsing lockfile: {}", err),
    }
}
