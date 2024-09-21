pub fn bytes_to_hex_string(bytes: &[u8]) -> String {
    static HEX_LUT: [char; 16] = [
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
    ];
    let mut out = String::new();
    for v in bytes {
        let upper = HEX_LUT[(v >> 4) as usize];
        let lower = HEX_LUT[(v & 0xF) as usize];
        out.push(upper);
        out.push(lower);
    }
    out
}
