import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App.tsx'
import './global.css'

window.addEventListener(
  "contextmenu",
  (event) => {
    event.preventDefault()
  },
  { capture: true },
)

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)
