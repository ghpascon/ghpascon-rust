pub const SIMPLE_ZPL_EXAMPLE: &str = "^XA\n^RS8\n^SZ2\n^FO10,10^BY3^BCN,60,Y,N,N^FD000000000000000000000001^FS\n^RFW,H^FD000000000000000000000001^FS\n^XZ";
pub const PARAMS_ZPL_EXAMPLE: &str = "^XA\n^RS8\n^SZ2\n^FO10,10^BY3^BCN,60,Y,N,N^FD{sequential}^FS\n^RFW,H^FD{epc}^FS\n^XZ";

pub fn generate_zpl_with_params(template: &str, params: &[(&str, &str)]) -> String {
    let mut result = template.to_string();
    for (key, value) in params {
        result = result.replace(&format!("{{{}}}", key), value);
    }
    result
}
