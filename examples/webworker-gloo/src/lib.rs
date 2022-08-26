use gloo_worker::{HandlerId, Worker, WorkerScope};

pub struct Multiplier {}

impl Worker for Multiplier {
    type Input = (u64, u64);
    type Message = ();
    type Output = ((u64, u64), u64);

    fn create(_scope: &WorkerScope<Self>) -> Self {
        Self {}
    }

    fn update(&mut self, _scope: &WorkerScope<Self>, _msg: Self::Message) {}

    fn received(&mut self, scope: &WorkerScope<Self>, msg: Self::Input, id: HandlerId) {
        scope.respond(id, (msg, msg.0 * msg.1));
    }
}
