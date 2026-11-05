# User guide

By the end of this page you can scan kegs, run batch operations, follow a lot and manage a brewery.

kegtrack tracks returnable kegs by QR code: every keg carries a label and every field operation is a scan. A brewery is the typical example, but nothing is specific to beer.

## Know the three roles

| Role | What it can do |
|---|---|
| **Read** | Look and export. No writes. |
| **Production** | Scan, create and validate operations, edit kegs, create customers and lots, edit a lot. |
| **Administrator** | Everything in its brewery: users, referentials, settings, forced transitions. |

These checks are applied **server-side on every route**. A hidden button is not a protection: a forbidden page stays forbidden even if you know its address.

## Sign in

Email address and password. The session lasts **30 days** on the device. This is on purpose: typing a strong password with wet gloves twenty times a day is not workable.

After 15 minutes of inactivity the session **locks** and asks for a six-digit PIN. The PIN never replaces the password: it wakes a session already open on a known device. After five wrong PINs you must sign in again completely.

Set your PIN in **My account**. It is what makes the application usable in the workshop. The PIN is typed on a **touch pad** with large keys, without the system keyboard.

## Follow the keg lifecycle

```
(commissioning) → Received → Dirty → Clean → Filled → Delivered ─┐
                     ▲                                           │
                     └──────────── back to the brewery ──────────┘
```

| Status | Meaning |
|---|---|
| **Received** | Back at the brewery, not sorted yet. Also the state of a new keg. |
| **Dirty** | Empty and soiled, waiting to be washed |
| **Clean** | Washed, available for filling |
| **Filled** | Holds a beer, a lot, a tank |
| **Delivered** | At a customer |
| **Out of service** | Removed from the fleet, repairable |
| **Lost** | Gone, with its history kept |

Allowed transitions:

| From | To |
|---|---|
| Received | Dirty, Clean |
| Dirty | Clean |
| Clean | Filled, Dirty |
| Filled | Delivered, Dirty |
| Delivered | Received (returned to the brewery), Dirty |
| Out of service | Dirty, Clean (repaired) |
| Lost | Received (found), Dirty |
| any status | Out of service, Lost |

An inconsistent transition is refused with a clear message: "a Dirty keg cannot be delivered". Some transitions need data: **Filled** needs a beer, a lot and a tank; **Delivered** needs a customer and an address; taking a keg out of the fleet needs a justification. The exact table lives in the `domaine` crate.

A keg that comes back **full** keeps its content: going to Received frees the customer, not the beer. Going to Dirty empties everything.

### Force a transition

Reality in the workshop always goes beyond the model. An administrator can force any transition, for example delivering a clean keg as an exchange. Forcing takes **two steps**: *Force (admin)* opens the form, which requires a justification, then a second validation confirms it. It shows in red in the keg's timeline, for good. That is the point: the exception leaves a trace instead of blending into the normal flow.

## Scan a keg

The **Scan** screen accepts three inputs, in order of efficiency in the field.

1. **The Bluetooth handheld scanner.** A 2D scanner for about forty euros behaves exactly like a keyboard. It works with nothing to configure: the input field is always ready, even after a click elsewhere. By far the fastest mode with gloves.
2. **The camera.** It starts by itself on mobile, on the rear camera. The *Torch* button appears if the device supports it, *Other camera* if the device has several. If the camera is blocked or did not start, *Retry* restarts it. It needs HTTPS: see [HTTPS and camera](https-camera.md).
3. **Manual entry**, for a damaged label: it is **folded** under "Code won't read?" to leave room for the viewfinder.

Every read gives three simultaneous feedbacks: full-screen colour, sound and vibration, because the operator does not watch the screen all the time.

- **Green:** keg found. The feedback is **short** and shows the **number** in capture (in a batch operation), then the scanner resumes by itself.
- **Red or amber:** unknown code, keg refused or already scanned. The screen **stays up until touched** ("Touch to continue"): a refusal must never go unnoticed. With a handheld scanner, the next read closes it too.

Scanning never creates a keg: **it finds it**. The fleet is pre-registered when the labels are made.

## Run an operation on many kegs

This is the core time saver: apply the same thing to dozens of kegs scanned in a row. Three steps, shown at the top of the screen: **1. Choose**, **2. Scan**, **3. Confirm**.

**1. Choose.** Touch a preset: *Washing done → Clean*, *Filling*, *Delivery*, *Customer return*. It prepares the operation type and the status. Fill in only what the case asks for (beer, lot and tank for a filling; customer and address for a delivery). *More settings* opens the full detail: status, beer, lot, tank, location, customer, address. Empty fields are not changed. **Type the comment now**: you will have nothing left to type afterwards. From a customer page or a lot page, the links *Deliver kegs to this customer* and *Fill kegs with this lot* arrive here already filled in.

**2. Scan.** Scan in a burst, with no validation between two kegs. The counter is always visible, and *Finish (N kegs)* moves on.

- A keg already scanned raises a warning, never a duplicate.
- An unknown code shows in red, is not added, and the scan goes on.
- **A keg that cannot take the requested operation is refused immediately**, while you hold it, not ten minutes later at the validation screen when it is too late to fetch it.
- *More options* offers **Remove all** (empty the list) and **Abandon** (throw the draft away). Each asks for confirmation in **two steps**: a first touch arms the button, "Yes, …" confirms.

Every scan is stored **server-side**. Refresh, phone asleep, dead battery, change of device: come back to the page and your capture is there.

**3. Confirm.** A summary says "X kegs will become Y". Validation is **transactional**: everything passes, or nothing is written. If a keg changed state in the meantime, the whole operation is refused with the reason, and nothing is modified.

For a delivery, the **printable delivery note** is generated at once, with two signature fields.

## Search everything

The search field on the desktop (sidebar) finds a keg, a customer or a lot in one entry. Press **/** anywhere outside an input field to put the cursor there.

## Browse the fleet

Full-text search (QR code, lot, customer), combinable filters, sorting by column (including by location), pagination. The columns *In this status since* and *Last movement* tell a sleeping keg from a moving one. The **CSV or Excel export reflects exactly the active filters**: the whole view, not the displayed page.

## Read a keg page

It centres on the **next action**: the large button proposes the logical next step of the cycle (for a Dirty keg: wash it). Other allowed transitions are under **Other action…**. Nothing else is offered: the buttons shown are the transitions actually allowed. Then come its current state, its **full timeline** since commissioning and the other kegs held by the same customer. *Reprint label* produces a replacement label for this keg.

## Manage customers

A customer is created with a first address in the same form. When a customer has only one address, it is preselected everywhere.

**A customer who holds kegs cannot be archived:** get them back first. And a customer is never deleted, only archived: a deletion would leave orphan kegs and break traceability.

The customer page shows the kegs held with the age of each. Beyond 30 days (configurable threshold), the age turns red.

## Follow lots and recall a product

A lot groups the kegs of one brew: number, brewing date, beer, tank, best-before date. The list filters by **beer** and by **near best-before date**.

The **lot page** lets you edit it (Production role and above), shows the attached kegs and their split by status, and offers *Fill kegs with this lot*, which opens the batch operation already filled in.

The **Product recall** screen answers in one click the question that matters in an incident: *who received lot 2026-017?* It lists every customer concerned, their contacts, the number of kegs received, those still held, delivery dates and the exact QR codes. Exportable and printable.

Lots whose best-before date is near show up automatically on the dashboard.

## Read the dashboard

It differs by who is looking.

- **Office** (Administrator, Read): clickable counters by status, kegs not returned beyond the threshold, best-before alerts, recent activity, fleet split, average time at a customer, and **quick actions** (deliver, fill, create a customer or a lot) depending on the role.
- **Workshop** (Production): two large buttons, Scan and Many kegs at once, and the two numbers that matter. Everything else disappears.

## Administer a brewery

Every sub-page has a **← Administration** link back. Destructive actions (archive, deactivate, change a role, and so on) ask for confirmation.

- **Referentials:** beer types, locations, tanks. These three lists feed every dropdown in the application. None of these fields is free text, which keeps statistics right.
- **Users:** creation with a generated temporary password, role change, activation. An administrator can neither demote nor deactivate themselves, and a brewery always keeps at least one active administrator.
- **QR labels:** generate a range, create the matching kegs, print a sheet for a 3 x 8 layout of 70 x 37 mm labels.
- **Import:** load an existing fleet from a spreadsheet. Columns: `code_qr`, `contenance`, and optionally `statut`, `client`, `emplacement`. A **CSV template** can be downloaded from the page. The file is **fully validated before any write**: one error and nothing is written, with a line-by-line detail. Past history does not exist: it starts at the import.
- **Settings:** QR prefix, default location, alert thresholds, allowed capacities, time zone.
- **Audit log:** who did what, when. Filterable by user and action, 50 entries per page.

## Use it on a phone

The interface changes shape below 720 px, it does not just shrink:

- lists become **cards**: code and status on the first line, then the filled fields. Empty fields disappear;
- on a keg page, **actions come before the timeline**: they would otherwise be under sixty events;
- secondary fleet filters are **folded** behind "More filters", so the first result is visible on the first screen;
- all touch targets are **at least 44 px**, and input fields use a 16 px font: below that, iOS zooms by itself on every tap;
- the bottom bar depends on the role. **Production:** Home, a large central **Scan** button, and "Many kegs". Administrator: home, fleet, scan, delivery notes, customers. Read: home, fleet, delivery notes, customers.

The interface uses the **dark** theme by default. The **light** theme is switched on in the menu ("Switch to light theme") and the choice is remembered on the device. Delivery notes and labels always print on a white background.

A tool checks that no page overflows horizontally, on iPhone 13, Pixel 5 and a 320 px screen: `outils/audit-mobile.mjs`, replayed by continuous integration.

## Buy the right labels

!!! warning "Paper labels do not survive a caustic wash"
    A paper QR label does not survive a wash with caustic soda at 80 °C. Plan for **engraved plates** or **industrial polyester labels**. It is the main friction point in real operation, and it is settled at purchase, not in software.
