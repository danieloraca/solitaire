const canvas = document.querySelector("#game");
const ctx = canvas.getContext("2d");
const statusEl = document.querySelector("#status");
const newGameButton = document.querySelector("#new-game");

const W = 1100;
const H = 720;
const CARD_W = 82;
const CARD_H = 118;
const TOP = 34;
const LEFT = 34;
const GAP = 22;
const TABLEAU_TOP = 188;
const SUITS = ["\u2660", "\u2665", "\u2666", "\u2663"];
const RANKS = ["", "A", "2", "3", "4", "5", "6", "7", "8", "9", "10", "J", "Q", "K"];

let wasm = null;
let drag = null;

function foundationX(index) {
  return LEFT + (CARD_W + GAP) * (3 + index);
}

function tableauX(index) {
  return LEFT + (CARD_W + GAP) * index;
}

function stockX() {
  return LEFT;
}

function wasteX() {
  return LEFT + CARD_W + GAP;
}

function newSeed() {
  const data = new Uint32Array(1);
  crypto.getRandomValues(data);
  return data[0] || Date.now();
}

async function load() {
  const response = await fetch("./dist/solitaire.wasm");
  if (!response.ok) {
    throw new Error("Build the WASM first: make build");
  }
  const bytes = await response.arrayBuffer();
  const module = await WebAssembly.instantiate(bytes, {});
  wasm = module.instance.exports;
  wasm.new_game(newSeed());
  draw();
}

function draw() {
  drawFelt();
  drawSlots();
  drawCards();
  drawHud();
  requestAnimationFrame(draw);
}

function drawFelt() {
  ctx.clearRect(0, 0, W, H);
  const felt = ctx.createLinearGradient(0, 0, W, H);
  felt.addColorStop(0, "#1c8157");
  felt.addColorStop(0.58, "#105f41");
  felt.addColorStop(1, "#0f422f");
  ctx.fillStyle = felt;
  ctx.fillRect(0, 0, W, H);

  ctx.save();
  ctx.globalAlpha = 0.09;
  ctx.strokeStyle = "#f5e7b0";
  ctx.lineWidth = 1;
  for (let x = -H; x < W; x += 28) {
    ctx.beginPath();
    ctx.moveTo(x, 0);
    ctx.lineTo(x + H, H);
    ctx.stroke();
  }
  ctx.restore();
}

function drawSlots() {
  drawSlot(stockX(), TOP, wasm.stock_count() > 0 ? "DECK" : "RESET");
  drawSlot(wasteX(), TOP, "WASTE");
  for (let i = 0; i < 4; i += 1) {
    drawSlot(foundationX(i), TOP, SUITS[i]);
  }
  for (let i = 0; i < 7; i += 1) {
    drawSlot(tableauX(i), TABLEAU_TOP, "");
  }

  if (wasm.stock_count() > 0) {
    drawBack(stockX(), TOP, false);
  }
}

function drawSlot(x, y, label) {
  ctx.save();
  roundRect(x, y, CARD_W, CARD_H, 8);
  ctx.fillStyle = "rgba(0, 0, 0, 0.12)";
  ctx.fill();
  ctx.strokeStyle = "rgba(246, 241, 223, 0.34)";
  ctx.setLineDash([8, 8]);
  ctx.lineWidth = 2;
  ctx.stroke();
  ctx.setLineDash([]);

  if (label) {
    ctx.fillStyle = "rgba(246, 241, 223, 0.54)";
    ctx.font = label.length > 2 ? "700 12px Inter, system-ui" : "700 32px Georgia, serif";
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillText(label, x + CARD_W / 2, y + CARD_H / 2);
  }
  ctx.restore();
}

function drawCards() {
  const count = wasm.render_count();
  for (let i = 0; i < count; i += 1) {
    const selected = wasm.card_selected(i) === 1;
    const dragOffset = selected && drag?.moved ? dragOffsetForCard() : { x: 0, y: 0 };
    const x = wasm.card_x(i) + dragOffset.x;
    const y = wasm.card_y(i) + dragOffset.y;
    const faceUp = wasm.card_face_up(i) === 1;

    if (faceUp) {
      drawFace(x, y, wasm.card_rank(i), wasm.card_suit(i), selected);
    } else {
      drawBack(x, y, selected);
    }
  }
}

function dragOffsetForCard() {
  return {
    x: drag.current.x - drag.start.x,
    y: drag.current.y - drag.start.y,
  };
}

function drawFace(x, y, rank, suit, selected) {
  ctx.save();
  drawCardShadow(x, y, selected);
  roundRect(x, y, CARD_W, CARD_H, 8);
  ctx.fillStyle = "#fffaf0";
  ctx.fill();
  ctx.strokeStyle = selected ? "#f9d75e" : "rgba(31, 23, 12, 0.22)";
  ctx.lineWidth = selected ? 4 : 1.4;
  ctx.stroke();

  const red = suit === 1 || suit === 2;
  ctx.fillStyle = red ? "#c73535" : "#1f2530";
  ctx.textAlign = "left";
  ctx.textBaseline = "alphabetic";
  ctx.font = "800 22px Georgia, serif";
  ctx.fillText(RANKS[rank], x + 10, y + 28);
  ctx.font = "700 22px Georgia, serif";
  ctx.fillText(SUITS[suit], x + 10, y + 52);

  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.font = "700 46px Georgia, serif";
  ctx.fillText(SUITS[suit], x + CARD_W / 2, y + CARD_H / 2 + 8);

  ctx.translate(x + CARD_W, y + CARD_H);
  ctx.rotate(Math.PI);
  ctx.textAlign = "left";
  ctx.textBaseline = "alphabetic";
  ctx.font = "800 22px Georgia, serif";
  ctx.fillText(RANKS[rank], 10, 28);
  ctx.font = "700 22px Georgia, serif";
  ctx.fillText(SUITS[suit], 10, 52);
  ctx.restore();
}

function drawBack(x, y, selected) {
  ctx.save();
  drawCardShadow(x, y, selected);
  roundRect(x, y, CARD_W, CARD_H, 8);
  ctx.fillStyle = "#263957";
  ctx.fill();
  ctx.strokeStyle = selected ? "#f9d75e" : "rgba(255, 255, 255, 0.28)";
  ctx.lineWidth = selected ? 4 : 1.2;
  ctx.stroke();

  roundRect(x + 10, y + 10, CARD_W - 20, CARD_H - 20, 5);
  ctx.fillStyle = "#d84f4f";
  ctx.fill();
  ctx.strokeStyle = "rgba(255, 255, 255, 0.35)";
  ctx.stroke();

  ctx.strokeStyle = "rgba(255, 246, 216, 0.5)";
  ctx.lineWidth = 2;
  for (let line = -CARD_H; line < CARD_W; line += 14) {
    ctx.beginPath();
    ctx.moveTo(x + 10 + line, y + 10);
    ctx.lineTo(x + 10 + line + CARD_H, y + CARD_H - 10);
    ctx.stroke();
  }
  ctx.restore();
}

function drawCardShadow(x, y, selected) {
  ctx.shadowColor = selected ? "rgba(249, 215, 94, 0.45)" : "rgba(0, 0, 0, 0.3)";
  ctx.shadowBlur = selected ? 16 : 12;
  ctx.shadowOffsetY = selected ? 2 : 7;
}

function drawHud() {
  const won = wasm.won() === 1;
  statusEl.textContent = won
    ? `Won in ${wasm.moves_count()} moves`
    : `${wasm.moves_count()} moves - ${wasm.stock_count()} in stock, ${wasm.waste_count()} in waste`;

  if (!won) {
    return;
  }

  ctx.save();
  ctx.fillStyle = "rgba(8, 24, 19, 0.74)";
  ctx.fillRect(0, 0, W, H);
  ctx.fillStyle = "#fff4bd";
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.font = "800 56px Inter, system-ui";
  ctx.fillText("You cleared it", W / 2, H / 2 - 22);
  ctx.font = "600 22px Inter, system-ui";
  ctx.fillText(`${wasm.moves_count()} moves`, W / 2, H / 2 + 28);
  ctx.restore();
}

function roundRect(x, y, w, h, r) {
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.arcTo(x + w, y, x + w, y + h, r);
  ctx.arcTo(x + w, y + h, x, y + h, r);
  ctx.arcTo(x, y + h, x, y, r);
  ctx.arcTo(x, y, x + w, y, r);
  ctx.closePath();
}

function boardPoint(event) {
  const rect = canvas.getBoundingClientRect();
  return {
    x: ((event.clientX - rect.left) / rect.width) * W,
    y: ((event.clientY - rect.top) / rect.height) * H,
  };
}

canvas.addEventListener("pointerdown", (event) => {
  if (!wasm || event.button !== 0) {
    return;
  }

  event.preventDefault();
  canvas.setPointerCapture(event.pointerId);
  const point = boardPoint(event);
  drag = {
    pointerId: event.pointerId,
    start: point,
    current: point,
    moved: false,
  };
  wasm.click(point.x, point.y);
});

canvas.addEventListener("pointermove", (event) => {
  if (!drag || drag.pointerId !== event.pointerId) {
    return;
  }

  event.preventDefault();
  const point = boardPoint(event);
  drag.current = point;
  const dx = point.x - drag.start.x;
  const dy = point.y - drag.start.y;
  drag.moved = drag.moved || Math.hypot(dx, dy) > 5;
});

canvas.addEventListener("pointerup", (event) => {
  if (!drag || drag.pointerId !== event.pointerId) {
    return;
  }

  event.preventDefault();
  const point = boardPoint(event);
  const shouldDrop = drag.moved;
  drag = null;

  if (shouldDrop) {
    wasm.click(point.x, point.y);
  }
});

canvas.addEventListener("pointercancel", (event) => {
  if (drag?.pointerId === event.pointerId) {
    drag = null;
  }
});

newGameButton.addEventListener("click", () => {
  if (wasm) {
    wasm.new_game(newSeed());
  }
});

load().catch((error) => {
  statusEl.textContent = error.message;
  console.error(error);
});
