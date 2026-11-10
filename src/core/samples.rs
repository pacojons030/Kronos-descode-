//! Muestras educativas integradas en el Explorador de Archivos de KRONOS Code.
//! Cada muestra demuestra distintas técnicas de ofuscación léxica/sintáctica y cómo
//! el motor estático en Rust las resuelve sin ejecutar el código.

#[allow(dead_code)]
pub struct CodeSample {
    pub filename: &'static str,
    pub description: &'static str,
    pub code: &'static str,
}

#[allow(dead_code)]
pub const BUILTIN_SAMPLES: &[CodeSample] = &[
    CodeSample {
        filename: "ejemplo_ofuscado.py",
        description: "Ofuscación multicapa: chr(), hex, base64, tablas estáticas y ramas muertas",
        code: r#"import base64

# ==========================================================
# Muestra 1: Pipeline de Verificación con Ofuscación Multicapa
# ==========================================================

# 1. Cadenas ocultas con chr() decimal y hexadecimal
_0x1fa = chr(65) + chr(117) + chr(116) + chr(101) + chr(110) + chr(116) + chr(105) + chr(99) + chr(97) + chr(99) + chr(105) + chr(111) + chr(110)

# 2. Payloads en Hexadecimal y Base64
_0x2bb = bytes.fromhex("436f727265637461").decode("utf-8")
_0x3cc = base64.b64decode(b"VXN1YXJpb19QcmVtaXVt").decode("utf-8")

# 3. Tabla estática de permisos con índice constante
_roles_tabla = ["invitado", "analista_junior", "administrador_sistema"]
_0x4dd = _roles_tabla[2]

# 4. Plegado aritmético constante
_puerto_seguro = (4000 + 443)

# 5. Rama condicional opaca (Código Muerto)
if False:
    print("[ERROR] Trampa de ofuscacion inalcanzable")
    _payload_trampa = 0xDEADBEEF
else:
    token_sesion = _0x1fa + " " + _0x2bb
    print(token_sesion)
    print("Usuario: " + _0x3cc + " | Rol: " + _0x4dd)
"#,
    },
    CodeSample {
        filename: "xor_cipher_loader.py",
        description: "Cifrado XOR por comprensión de listas y plegado aritmético de constantes",
        code: r#"# ==========================================================
# Muestra 2: Desempaquetador XOR Estático y Constantes Matemáticas
# ==========================================================

# 1. Cifrado XOR estático con clave 42 (0x2A)
_0x90a1 = "".join([chr(c ^ 42) for c in [105, 70, 75, 88, 79, 21, 121, 79, 77, 95, 88, 69]])

# 2. Cadena XOR secundaria con clave 17
_0x90b2 = "".join([chr(x ^ 17) for x in [64, 114, 114, 116, 98, 126, 49, 82, 126, 127, 114, 116, 117, 120, 117, 126]])

# 3. Expresiones matemáticas ofuscadas (Constant Folding)
_Il1Il1 = (128 * 4) + (64 // 2)
_O0O0O0 = (255 ^ 170)

if 0:
    _Il1Il1 = 999999
else:
    mensaje_final = _0x90a1 + " -> " + _0x90b2
    print(mensaje_final)
    print(_Il1Il1)
"#,
    },
    CodeSample {
        filename: "opaque_control_flow.py",
        description: "Identificadores tipo código de barras (Il1O0), arrays estáticos y ramas opacas",
        code: r#"import base64

# ==========================================================
# Muestra 3: Ofuscación de Nombres (Barcode IDs) y Predicados Opacos
# ==========================================================

_Il11lI1l = chr(0x50) + chr(0x72) + chr(0x6f) + chr(0x74) + chr(0x6f) + chr(0x63) + chr(0x6f) + chr(0x6c) + chr(0x6f)
_O00OO00O = bytes.fromhex("544c535f76315f33").decode("utf-8")

# Array estático indexado para ocultar rutas de configuración
_endpoints = ["http://localhost:8080", "https://api.interno.edu/v2", "https://backup.interno.edu"]
_I1lI1lI1 = _endpoints[1]

# Cálculo de timeout ofuscado
_0xdead01 = (30 * 1000) - 5000

if not True:
    print("Rama fantasma inyectada por el ofuscador")
else:
    cabecera = _Il11lI1l + "_" + _O00OO00O
    print(cabecera)
    print(_I1lI1lI1)
"#,
    },
    CodeSample {
        filename: "auditoria_sinks_test.py",
        description: "Prueba de Guardarraíles: Detecta sinks peligrosos (eval / os.system) ocultos",
        code: r#"import base64

# ==========================================================
# Muestra 4: Prueba del Escáner Heurístico Anti-Abuso (Zero-Exec)
# Demuestra cómo KRONOS revela llamadas peligrosas ocultas sin ejecutarlas
# ==========================================================

# Comando oculto tras Base64 y concatenación chr()
_0x77aa = base64.b64decode(b"b3Muc3lzdGVtKCJ3aG9hbWkiKQ==").decode("utf-8")
_0x88bb = chr(101) + chr(118) + chr(97) + chr(108) + chr(40) + chr(34) + chr(50) + chr(43) + chr(50) + chr(34) + chr(41)

# Llamada directa en el script para verificar la alerta del auditor estático:
if False:
    print("Rama señuelo")
else:
    # El escáner de seguridad detectará eval( y os.system( en el reporte inferior:
    resultado_simulado = eval("10 + 20")
    os.system("echo Auditoria Estatica Segura")
    print(_0x77aa)
"#,
    },
];

#[allow(dead_code)]
pub fn get_sample(index: usize) -> &'static CodeSample {
    BUILTIN_SAMPLES.get(index).unwrap_or(&BUILTIN_SAMPLES[0])
}
