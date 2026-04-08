# API Specification - Deckblattscanner Backend

This document describes the endpoints and data structures for the stateless Rust backend, including the image processing pipeline and the room management system.

## 1. Image Processing Pipeline

### `POST /scan`
Processes an uploaded image to extract student information (first name, last name, matriculation number).

*   **Content-Type:** `multipart/form-data`
*   **Request Body:**
    *   `image`: Binary image file (JPEG, PNG, etc. Max 10MB).
*   **Workflow:**
    1.  `ImagePreProcessor`: Optimizes the image.
    2.  `QrCodeScanner`: Detects a QR code. If valid JSON is found, processing stops and returns the result immediately.
    3.  `OcrScanner`: Fallback if QR fails. Uses `ocrs` (Local) or a Mock provider.
*   **Response (JSON):**
    ```json
    {
      "status": "success",
      "data": {
        "first_name": "Max",
        "last_name": "Mustermann",
        "matriculation_number": "1234567"
      }
    }
    ```
    *OR (Partial result):*
    ```json
    {
      "status": "partial",
      "data": {
        "info": {
          "first_name": "Max",
          "last_name": null,
          "matriculation_number": "1234567"
        },
        "missing": ["last_name"]
      }
    }
    ```

---

## 2. Room Management System

### `POST /rooms/create`
Creates a new room and assigns the caller as the owner.

*   **Content-Type:** `application/json`
*   **Request Body:**
    ```json
    { "user_name": "Alice" }
    ```
*   **Response (JSON):**
    ```json
    {
      "code": "ABC123",
      "user_id": "uuid-v4-string"
    }
    ```

### `GET /ws/join/{code}`
WebSocket endpoint to join an existing room using its 6-character code.

*   **Protocol:** WebSocket (Upgrade)
*   **Logic:**
    *   Ownership: The first user (creator) is the owner.
    *   Transfer: If the owner leaves, ownership passes to the next member in the list.
    *   Lifecycle: The room is deleted when the last member leaves.
*   **Inbound Messages (Future/Optional):** JSON strings matching `RoomMessage` structure.
*   **Outbound Messages (JSON):**
    *   `Joined`: Sent when a new user joins.
        ```json
        { "type": "Joined", "payload": { "user": { "id": "uuid", "name": "User-1234" }, "members": [...], "is_owner": true } }
        ```
    *   `Left`: Sent when a user leaves.
        ```json
        { "type": "Left", "payload": { "user_id": "uuid", "new_owner_id": "optional-uuid" } }
        ```
    *   `JoinRequest`: (If approval enabled) Sent to the owner when someone tries to join.

---

## 3. Local Setup Requirements
*   **OCR Models:** The backend requires `.rten` model files in the `./models` directory:
    *   `text-detection.rten`
    *   `text-recognition.rten`
*   **Port:** Default server port is `3000`.

## 4. Internal Architecture (For Agents)
*   **`AppState`**: Shared state containing `ocr_provider` (Thread-safe OCR engine) and `room_manager` (Thread-safe DashMap of active rooms).
*   **`ScannerPipeline`**: Modular trait-based processing system in `src/pipeline/mod.rs`.
*   **`RoomManager`**: Handles all logic for member lists and ownership transfers in `src/room_manager.rs`.
