(function () {
    var ws = new WebSocket('ws://' + window.location.host + '/_trunk/ws');
    ws.onmessage = (ev) => {
        const msg = JSON.parse(ev.data);
        if (msg.reload) {
            window.location.reload();
        }
    };
})()
