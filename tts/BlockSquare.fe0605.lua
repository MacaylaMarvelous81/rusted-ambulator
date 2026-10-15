local address, session_id = ""

function onLoad(script_state)
	address = self.UI.getAttribute("address", "text")
	session_id = self.UI.getAttribute("sessionId", "text")

	local scale = self.getScale()

	self.UI.setAttribute("basePanel", "scale", tostring(1 / scale.x) .. " " .. tostring(1 / scale.z) .. " 1")
end

function onEndEditAddress(player, value, id)
	address = value
end

function onEndEditId(player, value, id)
	session_id = value
end

function onSubmitSession(player, value, id)
	if address == nil or session_id == nil then
		return print("The address and session id fields must be filled!")
	end

	self.UI.setAttribute("address", "interactable", false)
	self.UI.setAttribute("sessionId", "interactable", false)
	self.UI.setAttribute("submitSession", "interactable", false)

	-- TODO: url encode inputs (id and name)
	WebRequest.put(address .. "/session/" .. session_id .. "?name=" .. player.steam_name, "", function(request)
		if request.response_code == 201 then
			print("A session with id '" .. session_id .. "' was successfully created!")
		elseif request.is_error then
			print("Encountered a request error: " .. request.error)
		else
			print("Unexpected response: " .. request.response_code .. " " .. request.text)
		end

		if request.response_code ~= 201 then
			self.UI.setAttribute("address", "interactable", true)
			self.UI.setAttribute("sessionId", "interactable", true)
			self.UI.setAttribute("submitSession", "interactable", true)
		end
	end)
end
