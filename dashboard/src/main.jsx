import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App.jsx'
import '@fontsource/jetbrains-mono/400.css'; // Regular
import '@fontsource/jetbrains-mono/500.css'; // Medium
import '@fontsource/jetbrains-mono/700.css'; // Bold
import './index.css'

ReactDOM.createRoot(document.getElementById('root')).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
)
