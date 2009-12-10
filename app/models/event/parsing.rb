class Event < ActiveRecord::Base
  
  #parse parameters for a potential event from the bookmarklet
  def Event.parse(params)
    return Event.new
    
    
  end
end