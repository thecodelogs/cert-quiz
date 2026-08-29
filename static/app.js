/* elapsed timer — keeps the hidden field in step with the visible clock */
(function () {
  var out = document.getElementById('clock');
  var field = document.getElementById('elapsed');
  if (!out || !field) return;
  var seconds = Number(out.dataset.elapsed) || 0;
  function paint() {
    var m = String(Math.floor(seconds / 60)).padStart(2, '0');
    var s = String(seconds % 60).padStart(2, '0');
    out.textContent = m + ':' + s;
    field.value = seconds;
  }
  paint();
  setInterval(function () { seconds++; paint(); }, 1000);
})();

/* auth popup — one modal, switchable between login and signup */
(function () {
  var overlay = document.getElementById('authOverlay');
  var openBtn = document.getElementById('authOpen');
  if (!overlay || !openBtn) return;

  var tabs = { login: document.getElementById('tabLogin'), signup: document.getElementById('tabSignup') };
  var forms = { login: document.getElementById('formLogin'), signup: document.getElementById('formSignup') };
  var prompts = { login: document.getElementById('promptLogin'), signup: document.getElementById('promptSignup') };

  function showTab(tab) {
    Object.keys(tabs).forEach(function (key) {
      var active = key === tab;
      tabs[key].classList.toggle('border-ink', active);
      tabs[key].classList.toggle('text-ink', active);
      tabs[key].classList.toggle('border-transparent', !active);
      tabs[key].classList.toggle('text-muted', !active);
      forms[key].classList.toggle('hidden', !active);
      prompts[key].classList.toggle('hidden', !active);
    });
  }

  function open(tab) {
    overlay.classList.remove('hidden');
    overlay.classList.add('flex');
    showTab(tab || 'login');
  }

  function close() {
    overlay.classList.add('hidden');
    overlay.classList.remove('flex');
  }

  openBtn.addEventListener('click', function () { open('login'); });
  overlay.addEventListener('click', function (e) { if (e.target === overlay) close(); });
  document.addEventListener('keydown', function (e) { if (e.key === 'Escape') close(); });

  var closeBtn = document.getElementById('authClose');
  if (closeBtn) closeBtn.addEventListener('click', close);

  var tabLogin = document.getElementById('tabLogin');
  if (tabLogin) tabLogin.addEventListener('click', function () { showTab('login'); });

  var tabSignup = document.getElementById('tabSignup');
  if (tabSignup) tabSignup.addEventListener('click', function () { showTab('signup'); });

  var switchToSignup = document.getElementById('switchToSignup');
  if (switchToSignup) switchToSignup.addEventListener('click', function (e) { e.preventDefault(); showTab('signup'); });

  var switchToLogin = document.getElementById('switchToLogin');
  if (switchToLogin) switchToLogin.addEventListener('click', function (e) { e.preventDefault(); showTab('login'); });

  ['formLogin', 'formSignup'].forEach(function (id) {
    var form = document.getElementById(id);
    if (form) form.addEventListener('submit', function (e) { e.preventDefault(); close(); });
  });
})();
