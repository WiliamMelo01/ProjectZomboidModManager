document.addEventListener('DOMContentLoaded', () => {
  const list = document.getElementById('mod-list')
  list.innerHTML = '<div class="placeholder">No mods yet — implement scanner</div>'

  // placeholder interactivity
  const search = document.getElementById('search')
  search.addEventListener('input', () => {
    list.innerHTML = '<div class="placeholder">Filter: ' + search.value + '</div>'
  })
})
