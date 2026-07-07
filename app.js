const canvas = document.querySelector("#game");
let ctx = canvas.getContext("2d");
const statusEl = document.querySelector("#status");
const newGameButton = document.querySelector("#new-game");
const undoButton = document.querySelector("#undo");
const leaderboardEl = document.querySelector("#leaderboard");

const W = 1100;
const H = 780;
const CARD_W = 104;
const CARD_H = 150;
const CARD_PAD = 30;
const TOP = 30;
const LEFT = 34;
const GAP = 18;
const TABLEAU_TOP = 214;
const SUITS = ["\u2660", "\u2665", "\u2666", "\u2663"];
const RANKS = ["", "A", "2", "3", "4", "5", "6", "7", "8", "9", "10", "J", "Q", "K"];
const FELT_LIGHT = "#23845b";
const FELT_DARK = "#0b3829";
const GOLD = "#f2d36b";
const LEADERBOARD_KEY = "rust-solitaire-leaderboard";
const MAX_LEADERBOARD_ENTRIES = 5;

let wasm = null;
let drag = null;
let drawQueued = false;
let feltSprite = null;
let currentGameId = "";
let savedWinGameId = "";
const cardSpriteCache = new Map();

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
  const response = await fetch("./dist/solitaire.wasm?v=large-bicycle-3", {
    cache: "no-store",
  });
  if (!response.ok) {
    throw new Error("Build the WASM first: make build");
  }
  const bytes = await response.arrayBuffer();
  const module = await WebAssembly.instantiate(bytes, {});
  wasm = module.instance.exports;
  if (wasm.layout_version?.() !== 3) {
    throw new Error("Old WASM loaded. Run make build on the Pi and hard refresh the page.");
  }
  renderLeaderboard();
  startNewGame();
}

function startNewGame() {
  const seed = newSeed();
  currentGameId = `${Date.now()}:${seed}`;
  savedWinGameId = "";
  wasm.new_game(seed);
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
  updateControls();
}

function drawFelt() {
  if (!feltSprite) {
    feltSprite = buildFeltSprite();
  }

  ctx.drawImage(feltSprite, 0, 0);
}

function buildFeltSprite() {
  const sprite = document.createElement("canvas");
  sprite.width = W;
  sprite.height = H;

  const mainCtx = ctx;
  ctx = sprite.getContext("2d");
  drawFeltArt();
  ctx = mainCtx;

  return sprite;
}

function drawFeltArt() {
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
    drawCardSprite(stockX(), TOP, false, 0, 0, false);
  }
}

function drawSlot(x, y, label) {
  ctx.save();
  roundRect(x, y, CARD_W, CARD_H, 10);
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
    ctx.font = label.length > 2 ? "700 13px Inter, system-ui" : "700 36px Georgia, serif";
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillText(label, x + CARD_W / 2, y + CARD_H / 2);
  }
  ctx.restore();
}

function drawCards() {
  const count = wasm.render_count();
  const dragged = [];
  for (let i = 0; i < count; i += 1) {
    const selected = wasm.card_selected(i) === 1;
    if (selected && drag?.moved) {
      dragged.push(i);
      continue;
    }

    drawRenderCard(i, 0, 0, selected);
  }

  if (!dragged.length) {
    return;
  }

  const offset = dragOffsetForCard();
  for (const i of dragged) {
    drawRenderCard(i, offset.x, offset.y, true);
  }
}

function drawRenderCard(i, offsetX, offsetY, selected) {
    const x = wasm.card_x(i) + offsetX;
    const y = wasm.card_y(i) + offsetY;
    const faceUp = wasm.card_face_up(i) === 1;

    drawCardSprite(x, y, faceUp, wasm.card_rank(i), wasm.card_suit(i), selected);
}

function drawCardSprite(x, y, faceUp, rank, suit, selected) {
  const sprite = getCardSprite(faceUp, rank, suit, selected);
  ctx.drawImage(sprite, x - CARD_PAD, y - CARD_PAD);
}

function getCardSprite(faceUp, rank, suit, selected) {
  const key = `${faceUp ? "f" : "b"}:${rank}:${suit}:${selected ? "s" : "n"}`;
  const cached = cardSpriteCache.get(key);
  if (cached) {
    return cached;
  }

  const sprite = document.createElement("canvas");
  sprite.width = CARD_W + CARD_PAD * 2;
  sprite.height = CARD_H + CARD_PAD * 2;

  const mainCtx = ctx;
  ctx = sprite.getContext("2d");
  ctx.save();
  ctx.translate(CARD_PAD, CARD_PAD);
  if (faceUp) {
    drawFace(0, 0, rank, suit, selected);
  } else {
    drawBack(0, 0, selected);
  }
  ctx.restore();
  ctx = mainCtx;

  cardSpriteCache.set(key, sprite);
  return sprite;
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
  roundRect(x, y, CARD_W, CARD_H, 10);
  const paper = ctx.createLinearGradient(x, y, x + CARD_W, y + CARD_H);
  paper.addColorStop(0, "#fffdf5");
  paper.addColorStop(0.55, "#fbf2df");
  paper.addColorStop(1, "#eadcc2");
  ctx.fillStyle = paper;
  ctx.fill();

  ctx.strokeStyle = selected ? GOLD : "rgba(42, 31, 17, 0.24)";
  ctx.lineWidth = selected ? 4 : 1.2;
  ctx.stroke();

  roundRect(x + 6, y + 6, CARD_W - 12, CARD_H - 12, 6);
  ctx.strokeStyle = "rgba(255, 255, 255, 0.62)";
  ctx.lineWidth = 1;
  ctx.stroke();

  drawCorner(x + 11, y + 11, rank, suit, 0);
  drawCorner(x + CARD_W - 11, y + CARD_H - 11, rank, suit, Math.PI);
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
  ctx.font = "800 22px Georgia, serif";
  ctx.fillText(RANKS[rank], 0, 0);
  ctx.font = "700 21px Georgia, serif";
  ctx.fillText(SUITS[suit], 1, 25);
  ctx.restore();
}

function drawCardBody(x, y, rank, suit) {
  if (rank > 10) {
    drawCourtCard(x, y, rank, suit);
    return;
  }

  const layout = pipLayout(rank);
  for (const pip of layout) {
    drawSuit(x + pip[0] * CARD_W, y + pip[1] * CARD_H, pip[2] || 30, suit, pip[3] || 0);
  }
}

function drawCourtCard(x, y, rank, suit) {
  const color = suitColor(suit);
  const label = RANKS[rank];
  ctx.save();
  roundRect(x + 27, y + 34, CARD_W - 54, CARD_H - 68, 10);
  const badge = ctx.createLinearGradient(x + 27, y + 34, x + CARD_W - 27, y + CARD_H - 34);
  badge.addColorStop(0, "rgba(255, 255, 255, 0.42)");
  badge.addColorStop(1, "rgba(0, 0, 0, 0.08)");
  ctx.fillStyle = badge;
  ctx.fill();
  ctx.strokeStyle = "rgba(42, 31, 17, 0.18)";
  ctx.stroke();

  ctx.fillStyle = color;
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.font = "800 44px Georgia, serif";
  ctx.fillText(label, x + CARD_W / 2, y + CARD_H / 2 - 18);
  drawSuit(x + CARD_W / 2, y + CARD_H / 2 + 27, 32, suit, 0);
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
      return [[0.5, 0.5, 56]];
    case 2:
      return [[0.5, top], [0.5, bottom, 30, Math.PI]];
    case 3:
      return [[0.5, top], [0.5, mid], [0.5, bottom, 30, Math.PI]];
    case 4:
      return [[left, top], [right, top], [left, bottom, 30, Math.PI], [right, bottom, 30, Math.PI]];
    case 5:
      return [[left, top], [right, top], [0.5, mid], [left, bottom, 30, Math.PI], [right, bottom, 30, Math.PI]];
    case 6:
      return [[left, top], [right, top], [left, mid], [right, mid], [left, bottom, 30, Math.PI], [right, bottom, 30, Math.PI]];
    case 7:
      return [[left, top], [right, top], [0.5, upper], [left, mid], [right, mid], [left, bottom, 30, Math.PI], [right, bottom, 30, Math.PI]];
    case 8:
      return [[left, top], [right, top], [0.5, upper], [left, mid], [right, mid], [0.5, lower, 30, Math.PI], [left, bottom, 30, Math.PI], [right, bottom, 30, Math.PI]];
    case 9:
      return [[left, top], [right, top], [left, upper], [right, upper], [0.5, mid], [left, lower, 30, Math.PI], [right, lower, 30, Math.PI], [left, bottom, 30, Math.PI], [right, bottom, 30, Math.PI]];
    case 10:
      return [[left, top], [right, top], [0.5, 0.34], [left, upper], [right, upper], [left, lower, 30, Math.PI], [right, lower, 30, Math.PI], [0.5, 0.66, 30, Math.PI], [left, bottom, 30, Math.PI], [right, bottom, 30, Math.PI]];
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
  roundRect(x, y, CARD_W, CARD_H, 10);
  ctx.fillStyle = "#f9f2dc";
  ctx.fill();
  ctx.strokeStyle = selected ? GOLD : "rgba(35, 25, 14, 0.28)";
  ctx.lineWidth = selected ? 4 : 1.4;
  ctx.stroke();

  roundRect(x + 7, y + 7, CARD_W - 14, CARD_H - 14, 8);
  ctx.fillStyle = "#bf2634";
  ctx.fill();
  ctx.strokeStyle = "#8d1625";
  ctx.lineWidth = 1.2;
  ctx.stroke();

  ctx.save();
  ctx.beginPath();
  roundRect(x + 13, y + 13, CARD_W - 26, CARD_H - 26, 5);
  ctx.clip();
  ctx.fillStyle = "#d73542";
  ctx.fillRect(x + 13, y + 13, CARD_W - 26, CARD_H - 26);

  ctx.strokeStyle = "rgba(255, 244, 218, 0.58)";
  ctx.lineWidth = 1.15;
  for (let row = y + 15; row < y + CARD_H - 14; row += 12) {
    for (let col = x + 15; col < x + CARD_W - 14; col += 12) {
      ctx.beginPath();
      ctx.moveTo(col + 6, row + 1);
      ctx.lineTo(col + 11, row + 6);
      ctx.lineTo(col + 6, row + 11);
      ctx.lineTo(col + 1, row + 6);
      ctx.closePath();
      ctx.stroke();
    }
  }

  ctx.strokeStyle = "rgba(112, 18, 34, 0.45)";
  ctx.lineWidth = 1;
  for (let line = -CARD_H; line < CARD_W; line += 16) {
    ctx.beginPath();
    ctx.moveTo(x + 13 + line, y + 13);
    ctx.lineTo(x + 13 + line + CARD_H, y + CARD_H - 13);
    ctx.stroke();
  }
  ctx.restore();

  drawBackFlourishes(x, y);
  drawBackMedallion(x, y);
  ctx.restore();
}

function drawBackFlourishes(x, y) {
  ctx.save();
  ctx.strokeStyle = "rgba(255, 245, 222, 0.9)";
  ctx.lineWidth = 1.5;
  roundRect(x + 15, y + 15, CARD_W - 30, CARD_H - 30, 4);
  ctx.stroke();
  roundRect(x + 21, y + 21, CARD_W - 42, CARD_H - 42, 3);
  ctx.strokeStyle = "rgba(255, 245, 222, 0.55)";
  ctx.stroke();

  for (const sx of [-1, 1]) {
    for (const sy of [-1, 1]) {
      ctx.save();
      ctx.translate(x + CARD_W / 2 + sx * 28, y + CARD_H / 2 + sy * 44);
      ctx.scale(sx, sy);
      ctx.beginPath();
      ctx.arc(0, 0, 12, 0.15 * Math.PI, 1.15 * Math.PI);
      ctx.arc(13, 0, 12, 1.15 * Math.PI, 0.15 * Math.PI, true);
      ctx.stroke();
      ctx.restore();
    }
  }
  ctx.restore();
}

function drawBackMedallion(x, y) {
  ctx.save();
  ctx.translate(x + CARD_W / 2, y + CARD_H / 2);
  ctx.fillStyle = "rgba(255, 245, 215, 0.86)";
  ctx.strokeStyle = "rgba(35, 25, 14, 0.24)";
  ctx.lineWidth = 1.2;
  ctx.beginPath();
  ctx.ellipse(0, 0, 25, 31, 0, 0, Math.PI * 2);
  ctx.fill();
  ctx.stroke();
  ctx.strokeStyle = "#bf2634";
  ctx.lineWidth = 2;
  ctx.beginPath();
  ctx.ellipse(0, 0, 17, 22, 0, 0, Math.PI * 2);
  ctx.stroke();
  ctx.fillStyle = "#1d3155";
  ctx.font = "800 24px Georgia, serif";
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
  const score = wasm.score();
  statusEl.textContent = won
    ? `Won in ${wasm.moves_count()} moves - ${score} points`
    : `${wasm.moves_count()} moves - ${score} points - ${wasm.stock_count()} in stock, ${wasm.waste_count()} in waste`;

  if (!won) {
    return;
  }

  saveCompletedGame(score, wasm.moves_count());

  ctx.save();
  ctx.fillStyle = "rgba(8, 24, 19, 0.74)";
  ctx.fillRect(0, 0, W, H);
  ctx.fillStyle = "#fff4bd";
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.font = "800 56px Inter, system-ui";
  ctx.fillText("You cleared it", W / 2, H / 2 - 22);
  ctx.font = "600 22px Inter, system-ui";
  ctx.fillText(`${wasm.moves_count()} moves - ${score} points`, W / 2, H / 2 + 28);
  ctx.restore();
}

function updateControls() {
  undoButton.disabled = !wasm || wasm.can_undo() !== 1;
}

function saveCompletedGame(score, moves) {
  if (savedWinGameId === currentGameId) {
    return;
  }

  const entries = readLeaderboard();
  entries.push({
    score,
    moves,
    date: new Date().toISOString(),
  });
  entries.sort(compareScores);
  writeLeaderboard(entries.slice(0, MAX_LEADERBOARD_ENTRIES));
  savedWinGameId = currentGameId;
  renderLeaderboard();
}

function compareScores(a, b) {
  if (b.score !== a.score) {
    return b.score - a.score;
  }
  if (a.moves !== b.moves) {
    return a.moves - b.moves;
  }
  return new Date(a.date).getTime() - new Date(b.date).getTime();
}

function readLeaderboard() {
  try {
    const parsed = JSON.parse(localStorage.getItem(LEADERBOARD_KEY) || "[]");
    if (!Array.isArray(parsed)) {
      return [];
    }

    return parsed
      .filter((entry) => Number.isFinite(entry.score) && Number.isFinite(entry.moves))
      .map((entry) => ({
        score: entry.score,
        moves: entry.moves,
        date: typeof entry.date === "string" ? entry.date : new Date().toISOString(),
      }))
      .sort(compareScores)
      .slice(0, MAX_LEADERBOARD_ENTRIES);
  } catch {
    return [];
  }
}

function writeLeaderboard(entries) {
  try {
    localStorage.setItem(LEADERBOARD_KEY, JSON.stringify(entries));
  } catch {
    // Ignore private-mode or quota failures; the current game still works.
  }
}

function renderLeaderboard() {
  const entries = readLeaderboard();
  leaderboardEl.replaceChildren();

  if (!entries.length) {
    const empty = document.createElement("li");
    empty.className = "leaderboard-empty";
    empty.textContent = "No completed games yet";
    leaderboardEl.append(empty);
    return;
  }

  for (const entry of entries) {
    const item = document.createElement("li");
    const score = document.createElement("strong");
    const detail = document.createElement("span");
    const date = new Date(entry.date);

    score.textContent = `${entry.score} points`;
    detail.textContent = `${entry.moves} moves - ${Number.isNaN(date.getTime()) ? "Saved" : date.toLocaleDateString()}`;

    item.append(score, detail);
    leaderboardEl.append(item);
  }
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

canvas.addEventListener("dblclick", (event) => {
  if (!wasm) {
    return;
  }

  event.preventDefault();
  const point = boardPoint(event);
  drag = null;
  wasm.auto_move_to_foundation(point.x, point.y);
  scheduleDraw();
});

newGameButton.addEventListener("click", () => {
  if (wasm) {
    drag = null;
    startNewGame();
  }
});

undoButton.addEventListener("click", () => {
  if (wasm && wasm.undo() === 1) {
    drag = null;
    scheduleDraw();
  }
});

window.addEventListener("resize", scheduleDraw);

load().catch((error) => {
  statusEl.textContent = error.message;
  console.error(error);
});
