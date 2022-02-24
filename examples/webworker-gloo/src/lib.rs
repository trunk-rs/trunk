use gloo_worker::{HandlerId, Public, Worker, WorkerLink};

pub struct Multiplier {
    link: WorkerLink<Self>,
}

impl Worker for Multiplier {
    type Input = (u64, u64);
    type Message = ();
    type Output = ((u64, u64), u64);
    type Reach = Public<Self>;

    fn create(link: WorkerLink<Self>) -> Self {
        Self { link }
    }

    fn update(&mut self, _msg: Self::Message) {}

    fn handle_input(&mut self, msg: Self::Input, id: HandlerId) {
        self.link.respond(id, (msg, msg.0 * msg.1));
    }

    fn name_of_resource() -> &'static str {
        "worker.js"
    }
}
