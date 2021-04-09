use trunk_plugin::{Args, Output, trunk_plugin};

#[trunk_plugin]
pub fn main(args: Args) -> Output {
    let msg = format!("Hello from WASM\nYou passed the arguments: {:?}", args);

    let argument_list = args.user_arguments
        .iter()
        .map(|(k, v)| {
            format!("<li>{}: {}</li>", k, v)
        })
        .collect::<String>();

    let html = format!(
        r#"
        <div>
            <div style="width: 100vw; height: 100vh; background: #c29f21; display: flex; align-items: center; justify-content: center">
                <h1 style="color: #e5c5c0"=>Hello Trunk!</h1>
            </div>
            <div style="position: absolute; top: 0; left: 0">
                <h3>Plugin permissions: <b>{permissions:?}</b></h3>
                <h3>Plugin arguments:</h3>
                <ul>
                    {arguments}
                </ul>
            </div>
        </div>
        "#,
        permissions = args.permissions,
        arguments = argument_list
    );

    Output {
        msg: Some(msg),
        html: Some(html),
    }
}
