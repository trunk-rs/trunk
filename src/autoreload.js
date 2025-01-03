"use strict";

(function () {

    const address = '{{__TRUNK_ADDRESS__}}';
    const base = '{{__TRUNK_WS_BASE__}}';
    let protocol = '{{__TRUNK_WS_PROTOCOL__}}';
    protocol =
        protocol
            ? protocol
            : window.location.protocol === 'https:'
                ? 'wss'
                : 'ws';
    const url = protocol + '://' + address + base + '.well-known/trunk/ws';

    class Overlay {
        constructor() {
            // create an overlay
            this._overlay = document.createElement("div");
            const style = this._overlay.style;
            style.height = "100vh";
            style.width = "100vw";
            style.position = "fixed";
            style.top = "0";
            style.left = "0";
            style.backgroundColor = "rgba(222, 222, 222, 0.5)";
            style.fontFamily = "sans-serif";
            // not sure that's the right approach
            style.zIndex = "1000000";
            style.backdropFilter = "blur(1rem)";

            const container = document.createElement("div");
            // center it
            container.style.position = "absolute";
            container.style.top = "30%";
            container.style.left = "15%";
            container.style.maxWidth = "85%";

            this._title = document.createElement("div");
            this._title.innerText = "Build failure";
            this._title.style.paddingBottom = "2rem";
            this._title.style.fontSize = "2.5rem";

            this._message = document.createElement("div");
            this._message.style.whiteSpace = "pre-wrap";

            const icon= document.createElement("div");
            icon.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" fill="#dc3545" viewBox="0 0 16 16"><path d="M8.982 1.566a1.13 1.13 0 0 0-1.96 0L.165 13.233c-.457.778.091 1.767.98 1.767h13.713c.889 0 1.438-.99.98-1.767L8.982 1.566zM8 5c.535 0 .954.462.9.995l-.35 3.507a.552.552 0 0 1-1.1 0L7.1 5.995A.905.905 0 0 1 8 5zm.002 6a1 1 0 1 1 0 2 1 1 0 0 1 0-2z"/></svg>';
            this._title.prepend(icon);

            container.append(this._title, this._message);
            this._overlay.append(container);

            this._inject();
            window.setInterval(() => {
                this._inject();
            }, 250);
        }

        set reason(reason) {
            this._message.textContent = reason;
        }

        _inject() {
            if (!this._overlay.isConnected) {
                // prepend it
                document.body?.prepend(this._overlay);
            }
        }

    }

    class Client {
        constructor(url) {
            this.url = url;
            this.poll_interval = 5000;
            this._overlay = null;
        }

        start() {
            const ws = new WebSocket(this.url);
            ws.onmessage = (ev) => {
                const msg = JSON.parse(ev.data);
                switch (msg.type) {
                    case "reload":
                        this.reload();
                        break;
                    case "buildFailure":
                        this.buildFailure(msg.data)
                        break;
                }
            };
            ws.onclose = () => this.onclose();
        }

        onclose() {
            window.setTimeout(
                () => {
                    // when we successfully reconnect, we'll force a
                    // reload (since we presumably lost connection to
                    // trunk due to it being killed, so it will have
                    // rebuilt on restart)
                    const ws = new WebSocket(this.url);
                    ws.onopen = () => window.location.reload();
                    ws.onclose = () => this.onclose();
                },
                this.poll_interval);
        }

        reload() {
            window.location.reload();
        }

        buildFailure({reason}) {
            // also log the console
            console.error("Build failed:", reason);

            console.debug("Overlay", this._overlay);

            if (!this._overlay) {
                this._overlay = new Overlay();
            }
            this._overlay.reason = reason;
        }
    }

    new Client(url).start();

})()
