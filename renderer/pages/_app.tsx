import "../styles/globals.css";
import Head from "next/head";
import { AppProps } from "next/app";
import { Provider } from "jotai";
import { Toaster } from "@/components/ui/toaster";
import { Tooltip } from "react-tooltip";
import "@/lib/tauri-bridge";
import { useEffect } from "react";

const MyApp = ({ Component, pageProps }: AppProps) => {
  useEffect(() => {
    const theme =
      typeof window !== "undefined"
        ? window.localStorage.getItem("theme")
        : null;
    document.documentElement.classList.add("dark");
    document.documentElement.setAttribute("data-theme", theme ?? "rescayl");
  }, []);

  return (
    <>
      <Head>
        <title>Rescayl</title>
      </Head>
      <base href="./" />

      <Provider>
        <Component {...pageProps} />
        <Toaster />
        <Tooltip
          className="z-[999] max-w-sm break-words !bg-secondary"
          id="tooltip"
        />
      </Provider>
    </>
  );
};

export default MyApp;
