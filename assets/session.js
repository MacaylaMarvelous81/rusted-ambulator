const colorSelect = document.getElementById('color-select');
const hand = document.getElementById('hand');

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
        Array.from(colorSelect.getElementsByClassName('color-select-colors')).forEach((child) => {
            child.remove();
        });
        payload.colors.forEach((color) => {
            const selection = document.createElement('option');
            selection.value = color;
            selection.innerText = color;
            selection.classList.add('color-select-colors');
            colorSelect.appendChild(selection);
        });
        colorSelect.disabled = false;
    } else if (Object.hasOwn(message, 'Hand')) {
        const payload = message.Hand;
        colorSelect.disabled = false;

        Array.from(hand.children).forEach((child) => {
            child.remove();
        });

        payload.forEach((handObject) => {
            if (Object.hasOwn(handObject, 'CustomDeck')) {
                const customDeck = handObject.CustomDeck;

                /*
                const cardWidthRatio = 100 / customDeck.width;
                const cardHeightRatio = 100 / customDeck.height;
                const cardHorizontalOffset = cardWidthRatio * (customDeck.card_id % customDeck.width);
                const cardVerticalOffset = cardHeightRatio * Math.floor(customDeck.card_id / customDeck.width);

                const card = document.createElement('img');
                card.src = customDeck.face;
                card.style.clipPath = `rect(${cardVerticalOffset}% ${cardWidthRatio}% ${cardVerticalOffset + cardHeightRatio}% ${cardHorizontalOffset}%`;
                card.style.objectFit = 'none';
                 */

                // 214x334
                const cardBounds = document.createElement('div');
                cardBounds.style.overflow = 'hidden';
                const cardFace = document.createElement('img');
                cardFace.src = customDeck.face;
                cardFace.addEventListener('load', () => {
                    const width = cardFace.naturalWidth / customDeck.width;
                    const height = cardFace.naturalHeight / customDeck.height;
                    const offsetX = width * (customDeck.card_id % customDeck.width);
                    const offsetY = height * Math.floor(customDeck.card_id / customDeck.width);

                    cardBounds.style.width = `${width}px`;
                    cardBounds.style.height = `${height}px`;
                    cardBounds.style.scale = '0.8';

                    cardFace.style.marginLeft = `-${offsetX}px`;
                    cardFace.style.marginTop = `-${offsetY}px`;
                });
                cardBounds.appendChild(cardFace);
                hand.appendChild(cardBounds);
            }
        });
    }
});

websocket.addEventListener('close', () => {
    colorSelect.disabled = true;
});

colorSelect.addEventListener('change', () => {
    if (colorSelect.value === '') return;

    colorSelect.disabled = true;

    websocket.send(JSON.stringify({
        "Color": colorSelect.value
    }));
});