import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { StreamProvider } from "../context/streamContext"; // Import the provider
import "./index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <StreamProvider>
      <App />
    </StreamProvider>
  </React.StrictMode>
);
