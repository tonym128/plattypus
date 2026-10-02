/**
 * Plattypus PS1 Web Arcade Companion Controller & Emulator Bridge
 */

document.addEventListener('DOMContentLoaded', () => {
  const iframe = document.getElementById('emulator-frame');
  const crtOverlay = document.getElementById('crt-overlay');
  const crtSelect = document.getElementById('crt-select');
  const discSelect = document.getElementById('disc-select');
  const btnReset = document.getElementById('btn-reset');
  const btnMute = document.getElementById('btn-mute');
  const btnFullscreen = document.getElementById('btn-fullscreen');
  const statusLed = document.getElementById('status-led');
  const statusText = document.getElementById('status-text');
  const gamepadStatus = document.getElementById('gamepad-status');
  const gamepadName = document.getElementById('gamepad-name');

  let isMuted = false;
  let isRunning = false;

  // 1. PostMessage Bridge to PSoXide Player
  function sendEmulatorCommand(command) {
    if (iframe && iframe.contentWindow) {
      iframe.contentWindow.postMessage(
        { type: 'psoxide-command', command: command },
        window.location.origin
      );
    }
  }

  window.addEventListener('message', (e) => {
    if (e.origin !== window.location.origin) return;
    const data = e.data;
    if (!data || data.type !== 'psoxide-event') return;

    console.log('[PSoXide Event]', data.event, data.message || '');
    switch (data.event) {
      case 'ready':
        statusLed.style.background = '#f59e0b';
        statusLed.style.boxShadow = '0 0 8px #f59e0b';
        statusText.textContent = 'READY (CLICK SCREEN TO START)';
        break;
      case 'starting':
        statusLed.style.background = '#3b82f6';
        statusLed.style.boxShadow = '0 0 8px #3b82f6';
        statusText.textContent = 'BOOTING MIPS R3000...';
        break;
      case 'running':
        isRunning = true;
        statusLed.style.background = '#10b981';
        statusLed.style.boxShadow = '0 0 8px #10b981';
        statusText.textContent = 'RUNNING (60 FPS)';
        break;
      case 'paused':
        isRunning = false;
        statusLed.style.background = '#f59e0b';
        statusLed.style.boxShadow = '0 0 8px #f59e0b';
        statusText.textContent = 'PAUSED';
        break;
      case 'error':
        statusLed.style.background = '#ef4444';
        statusLed.style.boxShadow = '0 0 8px #ef4444';
        statusText.textContent = `ERROR: ${data.message || 'EMULATION HALTED'}`;
        break;
    }
  });

  // 2. Toolbar Actions
  if (btnReset) {
    btnReset.addEventListener('click', () => {
      sendEmulatorCommand('reset');
      statusText.textContent = 'RESETTING...';
    });
  }

  if (btnMute) {
    btnMute.addEventListener('click', () => {
      isMuted = !isMuted;
      sendEmulatorCommand(isMuted ? 'mute' : 'unmute');
      btnMute.textContent = isMuted ? '🔇 Unmute' : '🔊 Mute';
      btnMute.classList.toggle('active', isMuted);
    });
  }

  if (btnFullscreen) {
    btnFullscreen.addEventListener('click', () => {
      const container = document.querySelector('.screen-container');
      if (!document.fullscreenElement) {
        if (container.requestFullscreen) container.requestFullscreen();
        else if (container.webkitRequestFullscreen) container.webkitRequestFullscreen();
      } else {
        if (document.exitFullscreen) document.exitFullscreen();
      }
    });
  }

  // 3. CRT Shader Selection
  if (crtSelect) {
    crtSelect.addEventListener('change', (e) => {
      const mode = e.target.value;
      crtOverlay.className = 'crt-overlay';
      if (mode === 'scanlines') {
        crtOverlay.classList.add('crt-scanlines');
      } else if (mode === 'trinitron') {
        crtOverlay.classList.add('crt-trinitron');
      }
    });
  }

  // 4. Disc Image Switcher
  if (discSelect) {
    discSelect.addEventListener('change', (e) => {
      const discPath = e.target.value;
      iframe.src = `player/?embed=1&disc=${discPath}`;
      statusLed.style.background = '#f59e0b';
      statusText.textContent = 'RELOADING DISC...';
    });
  }

  // 5. Interactive Tab Navigation
  const tabBtns = document.querySelectorAll('.tab-btn');
  const tabContents = document.querySelectorAll('.tab-content');

  tabBtns.forEach((btn) => {
    btn.addEventListener('click', () => {
      const targetId = btn.getAttribute('data-tab');

      tabBtns.forEach((b) => b.classList.remove('active'));
      tabContents.forEach((c) => c.classList.remove('active'));

      btn.classList.add('active');
      const targetEl = document.getElementById(targetId);
      if (targetEl) targetEl.classList.add('active');
    });
  });

  // 6. Gamepad API & Virtual Controller Highlighting
  const svgElements = {
    cross: document.getElementById('svg-cross'),
    circle: document.getElementById('svg-circle'),
    square: document.getElementById('svg-square'),
    triangle: document.getElementById('svg-triangle'),
    up: document.getElementById('svg-dpad-up'),
    down: document.getElementById('svg-dpad-down'),
    left: document.getElementById('svg-dpad-left'),
    right: document.getElementById('svg-dpad-right'),
    l1: document.getElementById('svg-l1'),
    r1: document.getElementById('svg-r1'),
    start: document.getElementById('svg-start'),
    select: document.getElementById('svg-select'),
    stickL: document.getElementById('svg-stick-l'),
    stickR: document.getElementById('svg-stick-r')
  };

  function setButtonState(name, active) {
    const el = svgElements[name];
    if (el) {
      if (active) {
        el.setAttribute('fill', '#00d2ff');
        el.setAttribute('filter', 'drop-shadow(0 0 6px #00d2ff)');
      } else {
        el.removeAttribute('fill');
        el.removeAttribute('filter');
      }
    }
  }

  // Keyboard mapping mirror for visualization
  const keyMap = {
    'KeyW': 'up',
    'ArrowUp': 'up',
    'KeyS': 'down',
    'ArrowDown': 'down',
    'KeyA': 'left',
    'ArrowLeft': 'left',
    'KeyD': 'right',
    'ArrowRight': 'right',
    'KeyK': 'cross',
    'KeyX': 'cross',
    'KeyL': 'circle',
    'KeyC': 'circle',
    'KeyJ': 'square',
    'KeyZ': 'square',
    'KeyI': 'triangle',
    'KeyV': 'triangle',
    'KeyQ': 'l1',
    'KeyE': 'r1',
    'Enter': 'start',
    'ShiftRight': 'select',
    'ShiftLeft': 'select',
    'Tab': 'select'
  };

  window.addEventListener('keydown', (e) => {
    const btn = keyMap[e.code];
    if (btn) setButtonState(btn, true);
  });

  window.addEventListener('keyup', (e) => {
    const btn = keyMap[e.code];
    if (btn) setButtonState(btn, false);
  });

  // Gamepad polling loop
  window.addEventListener('gamepadconnected', (e) => {
    if (gamepadStatus) gamepadStatus.classList.add('connected');
    if (gamepadName) gamepadName.textContent = e.gamepad.id.slice(0, 32);
  });

  window.addEventListener('gamepaddisconnected', () => {
    if (gamepadStatus) gamepadStatus.classList.remove('connected');
    if (gamepadName) gamepadName.textContent = 'No Gamepad Detected (Keyboard Ready)';
  });

  function pollGamepads() {
    const gamepads = navigator.getGamepads ? navigator.getGamepads() : [];
    if (gamepads && gamepads[0]) {
      const gp = gamepads[0];
      if (gamepadStatus && !gamepadStatus.classList.contains('connected')) {
        gamepadStatus.classList.add('connected');
        gamepadName.textContent = gp.id.slice(0, 32);
      }

      // Standard PS/Xbox button mapping
      setButtonState('cross', gp.buttons[0] && gp.buttons[0].pressed);
      setButtonState('circle', gp.buttons[1] && gp.buttons[1].pressed);
      setButtonState('square', gp.buttons[2] && gp.buttons[2].pressed);
      setButtonState('triangle', gp.buttons[3] && gp.buttons[3].pressed);
      setButtonState('l1', gp.buttons[4] && gp.buttons[4].pressed);
      setButtonState('r1', gp.buttons[5] && gp.buttons[5].pressed);
      setButtonState('select', gp.buttons[8] && gp.buttons[8].pressed);
      setButtonState('start', gp.buttons[9] && gp.buttons[9].pressed);
      setButtonState('up', gp.buttons[12] && gp.buttons[12].pressed || gp.axes[1] < -0.4);
      setButtonState('down', gp.buttons[13] && gp.buttons[13].pressed || gp.axes[1] > 0.4);
      setButtonState('left', gp.buttons[14] && gp.buttons[14].pressed || gp.axes[0] < -0.4);
      setButtonState('right', gp.buttons[15] && gp.buttons[15].pressed || gp.axes[0] > 0.4);

      if (svgElements.stickL && gp.axes.length >= 2) {
        const dx = Math.round(gp.axes[0] * 8);
        const dy = Math.round(gp.axes[1] * 8);
        svgElements.stickL.setAttribute('transform', `translate(${dx}, ${dy})`);
      }
    }
    requestAnimationFrame(pollGamepads);
  }
  requestAnimationFrame(pollGamepads);
});
