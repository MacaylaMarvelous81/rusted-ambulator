const colorSelect = document.getElementById('color-select');

const id = document.getElementById('session-script').dataset.id;
const websocket = new WebSocket(`/session/${id}/play`);

websocket.addEventListener('open', () => {
    websocket.send(JSON.stringify({
        "Initialize": { id }
    }));
});

websocket.addEventListener('message', (event) => {
    const message = JSON.parse(event.data);

    if (Object.hasOwn(message, 'Initialize')) {
        const payload = message.Initialize;
        for (let element in Array.from(colorSelect.getElementsByClassName('color-select-colors'))) {
            element.remove();
        }
        payload.colors.forEach((color) => {
            const selection = document.createElement('option');
            selection.value = color;
            selection.innerText = color;
            selection.classList.add('color-select-colors');
            colorSelect.appendChild(selection);
        });
        colorSelect.disabled = false;
    }
});

websocket.addEventListener('close', () => {
    colorSelect.disabled = true;
});

colorSelect.addEventListener('change', () => {
    colorSelect.disabled = true;
});