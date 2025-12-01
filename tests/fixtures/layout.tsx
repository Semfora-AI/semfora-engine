import { Outlet } from "react-router-dom";
import { useState } from "react";
import { Link } from "react-router-dom";

export default function AppLayout() {
  const [open, setOpen] = useState(false);

  return (
    <div>
      <header>
        <nav>
          <Link to="/a" />
          <Link to="/b" />
          <Link to="/c" />
          <Link to="/d" />
          <Link to="/e" />
          <Link to="/f" />
          <button onClick={() => setOpen(!open)}>Account</button>
          {open && <div>Sign out</div>}
        </nav>
      </header>
      <Outlet />
    </div>
  );
}
