use crate::components;
use yew::prelude::*;

#[function_component]
pub fn Login() -> Html {
    html! {
        <form>
            <components::input::Input label="Username" field_type="text" name="username" />
            <components::input::Input label="Password" field_type="password" name="password" />
            <button type="submit" class="btn btn-info w-100">{"Login"}</button>
        </form>
    }
}
