# Be sure to restart your server when you modify this file.

# Your secret key for verifying cookie session data integrity.
# If you change this key, all old sessions will become invalid!
# Make sure the secret is at least 30 characters and all random, 
# no regular words or you'll be exposed to dictionary attacks.
ActionController::Base.session = {
  :key         => '_riot_session',
  :secret      => 'b48a946097c5ef213af100c5376acb435b0b7f3fba5159f058f7286bb244d0993659fbf18316531df104a0f0b6851be9f81655f4bcfcad2665d251ac23a0779e'
}

# Use the database for sessions instead of the cookie-based default,
# which shouldn't be used to store highly confidential information
# (create the session table with "rake db:sessions:create")
# ActionController::Base.session_store = :active_record_store
