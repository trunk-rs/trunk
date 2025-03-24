# A basic project

For building the web application, `trunk` is running a combination of tools, mainly `cargo build` and `wasm-bindgen`.
Therefore, having a simple `cargo` project and an `index.html` file as an entry-point is sufficient to get you started.

## Creating a project

Start with creating a fresh Rust project and change into that folder:

```shell
cargo new trunk-hello-world
cd trunk-hello-world
```

Add some dependencies for the web:

```shell
cargo add wasm-bindgen console_error_panic_hook
cargo add web_sys -F Window,Document,HtmlElement,Text
```

Inside the newly created project, create a file `src/main.rs` with the following content:

```rust
use web_sys::window;

fn main() {
    console_error_panic_hook::set_once();

    let document = window()
        .and_then(|win| win.document())
        .expect("Could not access the document");
    let body = document.body().expect("Could not access document.body");
    let text_node = document.create_text_node("Hello, world from Vanilla Rust!");
    body.append_child(text_node.as_ref())
        .expect("Failed to append text");
}
```

Create an `index.html` in the root of project:

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8"/>
  <title>Hello World</title>
</head>
<body>
</body>
</html>
```

Then, start `trunk` inside that project to have it built and served:

```shell
trunk serve --open
```

This should compile the project, run `wasm_bindgen`, create an `index.html` based on the original file which loads and
initializes the application.

The application itself is pretty basic, simply getting the document's body and adding a note.

## Next steps

Most likely, you do not want to manually update the DOM tree of your application. You might want to add some assets,
tweak the build process, use some more browser APIs, perform some HTTP requests, use existing crates for the web, and
maybe even interface with the JavaScript world. However, all of this is an extension to this basic project we just
created.

Here are some pointers:

* [`wasm_bindgen` documentation](https://rustwasm.github.io/wasm-bindgen)
* HTML Frameworks (sorted by GitHub stars)
    * [Yew](https://yew.rs/)
    * [Leptos](https://github.com/gbj/leptos)
    * [Dioxus](https://dioxuslabs.com/)
    * [More](https://github.com/flosse/rust-web-framework-comparison?tab=readme-ov-file#frontend-frameworks-wasm)
* More `trunk` [examples](https://github.com/trunk-rs/trunk/tree/main/examples)
