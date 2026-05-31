const colorSelect = document.getElementById('color-select');

const id = document.getElementById('session-script').dataset.id;
const websocket = new WebSocket(`/session/${id}/play`);

websocket.addEventListener('open', () => {
    colorSelect.disabled = false;
});

colorSelect.addEventListener('change', () => {
    colorSelect.disabled = true;
});