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

