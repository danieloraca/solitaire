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
const FELT_LIGHT = "#23845b";
const FELT_DARK = "#0b3829";
const GOLD = "#f2d36b";

let wasm = null;
let drag = null;
let drawQueued = false;

function foundationX(index) {
  return W - LEFT - (4 - index) * (CARD_W + GAP);
}

function tableauX(index) {
  const spread = (W - LEFT * 2 - CARD_W) / 6;
  return LEFT + spread * index;
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
  const response = await fetch("./dist/solitaire.wasm?v=wide-layout-1", {
    cache: "no-store",
  });
  if (!response.ok) {
    throw new Error("Build the WASM first: make build");
  }
  const bytes = await response.arrayBuffer();
  const module = await WebAssembly.instantiate(bytes, {});
  wasm = module.instance.exports;
  if (wasm.layout_version?.() !== 1) {
    throw new Error("Old WASM loaded. Run make build on the Pi and hard refresh the page.");
  }
  wasm.new_game(newSeed());
  scheduleDraw();
}

function scheduleDraw() {
  if (drawQueued || !wasm) {
    return;
  }

  drawQueued = true;
  requestAnimationFrame(() => {
    drawQueued = false;
    draw();
  });
}

function draw() {
  drawFelt();
  drawSlots();
  drawCards();
  drawHud();
}

function drawFelt() {
  ctx.clearRect(0, 0, W, H);
  const felt = ctx.createLinearGradient(0, 0, W, H);
  felt.addColorStop(0, FELT_LIGHT);
  felt.addColorStop(0.55, "#115f43");
  felt.addColorStop(1, FELT_DARK);
  ctx.fillStyle = felt;
  ctx.fillRect(0, 0, W, H);

  ctx.save();
  ctx.globalAlpha = 0.075;
  ctx.strokeStyle = "#f7e8b6";
  ctx.lineWidth = 1;
  for (let x = -H; x < W; x += 24) {
    ctx.beginPath();
    ctx.moveTo(x, 0);
    ctx.lineTo(x + H, H);
    ctx.stroke();
  }
  ctx.globalAlpha = 0.05;
  for (let y = 0; y < H; y += 18) {
    ctx.beginPath();
    ctx.moveTo(0, y);
    ctx.lineTo(W, y + 28);
    ctx.stroke();
  }
  ctx.globalAlpha = 0.34;
  const glow = ctx.createRadialGradient(W * 0.48, H * 0.22, 80, W * 0.48, H * 0.22, 620);
  glow.addColorStop(0, "rgba(255, 242, 183, 0.24)");
  glow.addColorStop(0.45, "rgba(255, 242, 183, 0.06)");
  glow.addColorStop(1, "rgba(0, 0, 0, 0)");
  ctx.fillStyle = glow;
  ctx.fillRect(0, 0, W, H);
  const vignette = ctx.createRadialGradient(W / 2, H / 2, 250, W / 2, H / 2, 720);
  vignette.addColorStop(0, "rgba(0, 0, 0, 0)");
  vignette.addColorStop(1, "rgba(0, 0, 0, 0.36)");
  ctx.fillStyle = vignette;
  ctx.fillRect(0, 0, W, H);
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
  ctx.fillStyle = "rgba(2, 18, 12, 0.16)";
  ctx.fill();
  ctx.strokeStyle = "rgba(246, 241, 223, 0.32)";
  ctx.setLineDash([8, 8]);
  ctx.lineWidth = 2;
  ctx.stroke();
  ctx.setLineDash([]);

  ctx.strokeStyle = "rgba(0, 0, 0, 0.18)";
  ctx.lineWidth = 1;
  roundRect(x + 4, y + 4, CARD_W - 8, CARD_H - 8, 6);
  ctx.stroke();

  if (label) {
    ctx.fillStyle = "rgba(246, 241, 223, 0.5)";
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
  const paper = ctx.createLinearGradient(x, y, x + CARD_W, y + CARD_H);
  paper.addColorStop(0, "#fffdf5");
  paper.addColorStop(0.55, "#fbf2df");
  paper.addColorStop(1, "#eadcc2");
  ctx.fillStyle = paper;
  ctx.fill();

  ctx.strokeStyle = selected ? GOLD : "rgba(42, 31, 17, 0.24)";
  ctx.lineWidth = selected ? 4 : 1.2;
  ctx.stroke();

  roundRect(x + 5, y + 5, CARD_W - 10, CARD_H - 10, 5);
  ctx.strokeStyle = "rgba(255, 255, 255, 0.62)";
  ctx.lineWidth = 1;
  ctx.stroke();

  drawCorner(x + 9, y + 10, rank, suit, 0);
  drawCorner(x + CARD_W - 9, y + CARD_H - 10, rank, suit, Math.PI);
  drawCardBody(x, y, rank, suit);
  ctx.restore();
}

function drawCorner(x, y, rank, suit, rotation) {
  const color = suitColor(suit);
  ctx.save();
  ctx.translate(x, y);
  ctx.rotate(rotation);
  ctx.fillStyle = color;
  ctx.textAlign = "left";
  ctx.textBaseline = "top";
  ctx.font = "800 18px Georgia, serif";
  ctx.fillText(RANKS[rank], 0, 0);
  ctx.font = "700 17px Georgia, serif";
  ctx.fillText(SUITS[suit], 1, 20);
  ctx.restore();
}

function drawCardBody(x, y, rank, suit) {
  if (rank > 10) {
    drawCourtCard(x, y, rank, suit);
    return;
  }

  const layout = pipLayout(rank);
  for (const pip of layout) {
    drawSuit(x + pip[0] * CARD_W, y + pip[1] * CARD_H, pip[2] || 24, suit, pip[3] || 0);
  }
}

function drawCourtCard(x, y, rank, suit) {
  const color = suitColor(suit);
  const label = RANKS[rank];
  ctx.save();
  roundRect(x + 22, y + 28, CARD_W - 44, CARD_H - 56, 8);
  const badge = ctx.createLinearGradient(x + 22, y + 28, x + CARD_W - 22, y + CARD_H - 28);
  badge.addColorStop(0, "rgba(255, 255, 255, 0.42)");
  badge.addColorStop(1, "rgba(0, 0, 0, 0.08)");
  ctx.fillStyle = badge;
  ctx.fill();
  ctx.strokeStyle = "rgba(42, 31, 17, 0.18)";
  ctx.stroke();

  ctx.fillStyle = color;
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.font = "800 36px Georgia, serif";
  ctx.fillText(label, x + CARD_W / 2, y + CARD_H / 2 - 15);
  drawSuit(x + CARD_W / 2, y + CARD_H / 2 + 22, 26, suit, 0);
  ctx.restore();
}

function pipLayout(rank) {
  const left = 0.34;
  const right = 0.66;
  const top = 0.28;
  const upper = 0.38;
  const mid = 0.5;
  const lower = 0.62;
  const bottom = 0.72;

  switch (rank) {
    case 1:
      return [[0.5, 0.5, 44]];
    case 2:
      return [[0.5, top], [0.5, bottom, 24, Math.PI]];
    case 3:
      return [[0.5, top], [0.5, mid], [0.5, bottom, 24, Math.PI]];
    case 4:
      return [[left, top], [right, top], [left, bottom, 24, Math.PI], [right, bottom, 24, Math.PI]];
    case 5:
      return [[left, top], [right, top], [0.5, mid], [left, bottom, 24, Math.PI], [right, bottom, 24, Math.PI]];
    case 6:
      return [[left, top], [right, top], [left, mid], [right, mid], [left, bottom, 24, Math.PI], [right, bottom, 24, Math.PI]];
    case 7:
      return [[left, top], [right, top], [0.5, upper], [left, mid], [right, mid], [left, bottom, 24, Math.PI], [right, bottom, 24, Math.PI]];
    case 8:
      return [[left, top], [right, top], [0.5, upper], [left, mid], [right, mid], [0.5, lower, 24, Math.PI], [left, bottom, 24, Math.PI], [right, bottom, 24, Math.PI]];
    case 9:
      return [[left, top], [right, top], [left, upper], [right, upper], [0.5, mid], [left, lower, 24, Math.PI], [right, lower, 24, Math.PI], [left, bottom, 24, Math.PI], [right, bottom, 24, Math.PI]];
    case 10:
      return [[left, top], [right, top], [0.5, 0.34], [left, upper], [right, upper], [left, lower, 24, Math.PI], [right, lower, 24, Math.PI], [0.5, 0.66, 24, Math.PI], [left, bottom, 24, Math.PI], [right, bottom, 24, Math.PI]];
    default:
      return [];
  }
}

function drawSuit(cx, cy, size, suit, rotation) {
  ctx.save();
  ctx.translate(cx, cy);
  ctx.rotate(rotation);
  ctx.fillStyle = suitColor(suit);
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.font = `700 ${size}px Georgia, serif`;
  ctx.fillText(SUITS[suit], 0, 2);
  ctx.restore();
}

function suitColor(suit) {
  return suit === 1 || suit === 2 ? "#bd2d32" : "#161d27";
}

function drawBack(x, y, selected) {
  ctx.save();
  drawCardShadow(x, y, selected);
  roundRect(x, y, CARD_W, CARD_H, 8);
  const back = ctx.createLinearGradient(x, y, x + CARD_W, y + CARD_H);
  back.addColorStop(0, "#2d4e75");
  back.addColorStop(0.45, "#263957");
  back.addColorStop(1, "#142238");
  ctx.fillStyle = back;
  ctx.fill();
  ctx.strokeStyle = selected ? GOLD : "rgba(255, 255, 255, 0.3)";
  ctx.lineWidth = selected ? 4 : 1.2;
  ctx.stroke();

  roundRect(x + 9, y + 9, CARD_W - 18, CARD_H - 18, 5);
  ctx.strokeStyle = "rgba(255, 255, 255, 0.38)";
  ctx.lineWidth = 1.5;
  ctx.stroke();

  ctx.save();
  ctx.beginPath();
  roundRect(x + 12, y + 12, CARD_W - 24, CARD_H - 24, 4);
  ctx.clip();
  ctx.fillStyle = "#d64b4f";
  ctx.fillRect(x + 12, y + 12, CARD_W - 24, CARD_H - 24);
  ctx.strokeStyle = "rgba(255, 245, 215, 0.5)";
  ctx.lineWidth = 1.4;
  for (let row = y + 14; row < y + CARD_H - 12; row += 13) {
    for (let col = x + 12; col < x + CARD_W - 12; col += 13) {
      ctx.beginPath();
      ctx.moveTo(col + 6, row);
      ctx.lineTo(col + 12, row + 6);
      ctx.lineTo(col + 6, row + 12);
      ctx.lineTo(col, row + 6);
      ctx.closePath();
      ctx.stroke();
    }
  }
  ctx.restore();

  drawBackMedallion(x, y);
  ctx.restore();
}

function drawBackMedallion(x, y) {
  ctx.save();
  ctx.translate(x + CARD_W / 2, y + CARD_H / 2);
  ctx.fillStyle = "rgba(255, 245, 215, 0.86)";
  ctx.strokeStyle = "rgba(35, 25, 14, 0.24)";
  ctx.lineWidth = 1.2;
  ctx.beginPath();
  ctx.arc(0, 0, 18, 0, Math.PI * 2);
  ctx.fill();
  ctx.stroke();
  ctx.fillStyle = "#263957";
  ctx.font = "800 18px Georgia, serif";
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.fillText("\u2660", 0, 1);
  ctx.restore();
}

function drawCardShadow(x, y, selected) {
  ctx.shadowColor = selected ? "rgba(242, 211, 107, 0.5)" : "rgba(0, 0, 0, 0.34)";
  ctx.shadowBlur = selected ? 18 : 13;
  ctx.shadowOffsetY = selected ? 4 : 8;
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
  scheduleDraw();
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
  scheduleDraw();
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
  scheduleDraw();
});

canvas.addEventListener("pointercancel", (event) => {
  if (drag?.pointerId === event.pointerId) {
    drag = null;
    scheduleDraw();
  }
});

newGameButton.addEventListener("click", () => {
  if (wasm) {
    wasm.new_game(newSeed());
    scheduleDraw();
  }
});

window.addEventListener("resize", scheduleDraw);

load().catch((error) => {
  statusEl.textContent = error.message;
  console.error(error);
});
