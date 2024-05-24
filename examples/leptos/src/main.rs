use leptos::*;

#[component]
pub fn App() -> impl IntoView {
    view! {
        <html>
            <head>
                <title>My Leptos App</title>
            </head>
            <body>
                <p>"Hello, world!"</p>
                <script type="module" src="script.mjs"></script>
            </body>
        </html>
    }
}

fn main() {
    mount_to_body(|| view! { <App/> });
}
