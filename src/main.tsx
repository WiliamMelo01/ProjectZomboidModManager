import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App.tsx'
import './global.css'
import { initializeI18n } from './i18n'

window.addEventListener(
  "contextmenu",
  (event) => {
    event.preventDefault()
  },
  { capture: true },
)

void initializeI18n().then(() => {
  ReactDOM.createRoot(document.getElementById('root')!).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  )
})
