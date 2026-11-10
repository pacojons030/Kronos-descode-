# KRONOS AST Studio — Native Rust Static Engine

Aplicación de ingeniería de software comercial desarrollada íntegramente en **Rust** con el framework de interfaz gráfica declarativa de alto rendimiento **Slint**.

---

## 1. Identidad Visual y Filosofía de Diseño

- **Paleta Cromática Obsidian & Deep Slate:** Fondos carbón (`#090a0d`, `#0e1116`, `#0d1015`) con microcontrastes calculados para eliminar la fatiga visual.
- **Sin Clichés de IA:** Cero degradados morados o púrpuras, cero resplandores de neón, cero bordes redondeados excesivos.
- **Tipografía y Renderizado Vectorial Slint:** Renderizado por hardware mediante GPU con subpixel anti-aliasing y fuentes monoespaciadas para código (`Consolas` / `Segoe UI`).
- **Composición Equilibrada:**
  - **Barra de Comandos Superior:** Monograma KRONOS, breadcrumbs de ruta y acciones globales con feedback táctil y atajos (`Ctrl+O`, `Ctrl+S`, `F5`).
  - **Panel de Control Lateral:** Switches de palanca fluidos para cada pase del compilador y tarjetas de métricas de compilación con números en alta definición.
  - **Dual Code Buffers:** Marcos estructurados con cabeceras dedicadas, estados de foco y división balanceada.
  - **Barra de Telemetría Inferior:** Notificaciones de estado, versión del motor y metadatos de ejecución en tiempo real.

---

## 2. Arquitectura de Código Rust

```
kronos_ast/
├── Cargo.toml
├── build.rs              // Compilación nativa Slint en tiempo de build
├── ui/
│   └── app.slint         // Especificación de diseño, componentes y animaciones
└── src/
    ├── main.rs           // Enrutamiento de eventos, controladores de UI y ciclo de vida
    └── core/
        ├── mod.rs        // Fachada pública del motor
        ├── decoders.rs   // Decodificación de cadenas (chr, base64, hex, concatenaciones)
        ├── normalizer.rs // Normalización y remapeo de identificadores ofuscados
        └── pipeline.rs   // Orquestador multi-paso del AST y métricas
```

---

## 3. Ejecución

Solo haz doble clic sobre:
👉 **`KRONOS_AST.exe`**
